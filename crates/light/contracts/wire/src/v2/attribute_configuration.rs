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
    Position,
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
    #[serde(default)]
    pub push_turn_of: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeActivationGroup {
    pub id: String,
    pub label: String,
    pub members: Vec<String>,
}

/// How a show programs colour: fixture-native channels, or one device-independent Color Intent.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorProgrammingModel {
    #[default]
    Direct,
    Intent,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfiguration {
    pub version: u16,
    pub custom_attributes: Vec<CustomAttributeDescriptor>,
    pub placements: Vec<AttributePlacement>,
    pub activation_groups: Vec<AttributeActivationGroup>,
    /// Absent from requests written before Color Intent, which therefore mean Direct.
    #[serde(default)]
    pub color_model: ColorProgrammingModel,
}

/// What switching a programmed show to another colour model would do to its stored values.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorModelImpactKind {
    /// Fixture-native colour-channel values: kept and still played, but no longer edited from the
    /// Color feature in Intent.
    NativeColorValues,
    /// Whole-colour values whose brightness is carried in the colour; Intent shows them at full
    /// brightness and leaves dimming to Intensity.
    DimmedWholeColors,
    /// Whole-colour values on fixtures without an authored colour system; Direct cannot resolve
    /// them, so those fixtures lose that colour.
    UnresolvedWholeColors,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorModelImpactItem {
    pub kind: ColorModelImpactKind,
    pub count: u32,
    /// The stored values cannot come back unchanged if the operator switches back.
    pub lossy: bool,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorModelImpact {
    pub from: ColorProgrammingModel,
    pub to: ColorProgrammingModel,
    /// At least one item is lossy: the update must acknowledge the impact.
    pub lossy: bool,
    pub items: Vec<ColorModelImpactItem>,
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
    pub push_turn_of: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfigurationSnapshot {
    pub show_id: Option<Uuid>,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub object_revision: u64,
    pub configuration: AttributeConfiguration,
    pub recommended_configuration: AttributeConfiguration,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub color_model: Option<ColorProgrammingModel>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfigurationUpdateRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_show_revision: u64,
    #[ts(type = "number")]
    pub expected_object_revision: u64,
    pub patch: AttributeConfigurationPatch,
    /// Required when changing `color_model` would lose stored colour: the operator has seen the
    /// `ColorModelImpact` and chosen to switch anyway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub acknowledge_color_model_impact: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct AttributeConfigurationUpdateOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub snapshot: AttributeConfigurationSnapshot,
    #[ts(type = "number")]
    pub event_sequence: u64,
}

/// How faithfully one fixture head shows its Color Intent target.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorResolutionQuality {
    Exact,
    Approximate,
    OutOfGamut,
    WheelLimited,
    Uncalibrated,
    Unsupported,
}

/// The kind of colour engine that shows a head's Color Intent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentEngine {
    Additive,
    Subtractive,
    HueSaturation,
    Wheel,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorIntentHeadReport {
    pub fixture_id: Uuid,
    pub fixture_number: Option<u32>,
    pub fixture_name: String,
    /// The selectable identity owning the head: the fixture, or one of its logical heads.
    pub owner_id: Uuid,
    pub head_name: String,
    /// False when nothing programs a colour yet; the head is then reported against white.
    pub has_target: bool,
    pub quality: ColorResolutionQuality,
    pub engine: Option<ColorIntentEngine>,
    /// CIE 1976 u′v′ distance between the target and what the head is modelled to show.
    pub delta_uv: Option<f32>,
    /// The chosen colour system's calibration revision; absent for inferred systems.
    pub calibration_revision: Option<u32>,
    /// TL-550: the head's UV result, reported separately from the visible match. Present only
    /// in the accepted-frame report for a lamp head; absent for Media and the legacy report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub uv: Option<ColorIntentUvReport>,
    /// TL-554: the head's Direct replay status when it shows a Direct (native) value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub direct: Option<super::native_color::ColorIntentDirectReport>,
    /// TL-552: how the head shows colour without a profile colour model (controls parked at
    /// neutral, shared controls left at default, a nominal hue/saturation grid, white only), or
    /// why it has no colour model at all. Absent when there is nothing to say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub note: Option<String>,
}

/// What the fitter did with a head's requested UV, independent of the visible colour.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentUvStatus {
    /// No UV requested and the head has no UV emitter.
    NotRequested,
    /// Every UV emitter is driven at the requested amount (zero closes them).
    Applied,
    /// A UV amount is requested on a head without UV emitters; the request stays stored.
    Unsupported,
}

/// One head's UV result in an accepted output frame.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorIntentUvReport {
    pub status: ColorIntentUvStatus,
    /// The requested amount was limited by the emitter's drive range.
    pub clipped: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorIntentReport {
    pub color_model: ColorProgrammingModel,
    pub heads: Vec<ColorIntentHeadReport>,
    /// Present only when the report is read from the accepted output frame (the family
    /// adapters are engaged). Heads are then only those with an active requested colour in that
    /// frame; nothing is reported against an invented white. Absent for the legacy report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub accepted_frame: Option<ColorIntentAcceptedFrame>,
}

/// The accepted output frame a colour report was read from.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ColorIntentAcceptedFrame {
    pub state: ColorIntentFrameState,
    /// The published output frame whose Color results the heads report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub frame: Option<super::output_control::OutputFrameIdentity>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ColorIntentFrameState {
    /// `heads` come from the published frame's Color results.
    Accepted,
    /// No accepted output frame with Color results matches the published frame yet (or the
    /// patch moved on since). Passive: `heads` is empty; read again.
    NotYetAvailable,
}
