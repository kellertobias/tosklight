use crate::DirectControlProtocol;
use crate::profile::FixtureProfile;
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    MsbFirst,
    LsbFirst,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelComponent {
    pub offset: u16,
    pub byte_order: ByteOrder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub dmx_from: u8,
    pub dmx_to: u8,
    pub preset_family: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Parameter {
    pub attribute: AttributeKey,
    pub components: Vec<ChannelComponent>,
    /// Unspecified channel defaults are DMX zero for compatibility with minimal fixture profiles.
    #[serde(default)]
    pub default: f32,
    pub virtual_dimmer: bool,
    #[serde(default)]
    pub metadata: ParameterMetadata,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ParameterMetadata {
    pub physical_min: f32,
    pub physical_max: f32,
    pub unit: Option<String>,
    pub invert: bool,
    pub wrap: bool,
    pub curve: DmxCurve,
}
impl Default for ParameterMetadata {
    fn default() -> Self {
        Self {
            physical_min: 0.0,
            physical_max: 1.0,
            unit: None,
            invert: false,
            wrap: false,
            curve: DmxCurve::Linear,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DmxCurve {
    #[default]
    Linear,
    Square,
    SquareRoot,
    SmoothStep,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogicalHead {
    pub index: u16,
    pub name: String,
    #[serde(default)]
    pub shared: bool,
    pub parameters: Vec<Parameter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmitterCalibration {
    pub name: String,
    pub xyz: Xyz,
    pub limit: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorCalibration {
    pub emitters: Vec<EmitterCalibration>,
    #[serde(default = "identity_matrix")]
    pub correction_matrix: [[f32; 3]; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureDefinition {
    pub schema_version: u16,
    pub id: FixtureId,
    pub revision: u32,
    pub manufacturer: String,
    /// Broad, operator-facing classification used to browse the desk library.
    #[serde(default)]
    pub device_type: String,
    /// Human-readable fixture name. `model` remains the manufacturer's model identifier.
    #[serde(default)]
    pub name: String,
    pub model: String,
    pub mode: String,
    pub footprint: u16,
    pub heads: Vec<LogicalHead>,
    pub color_calibration: Option<ColorCalibration>,
    #[serde(default)]
    pub physical: FixturePhysicalProperties,
    /// Optional stage-view assets. Values are portable asset identifiers or data URLs.
    #[serde(default)]
    pub model_asset: Option<String>,
    #[serde(default)]
    pub icon_asset: Option<String>,
    pub hazardous: bool,
    /// Direct-control transports explicitly supported by this fixture profile.
    #[serde(default)]
    pub direct_control_protocols: Vec<DirectControlProtocol>,
    #[serde(default)]
    pub signal_loss_policy: SignalLossPolicy,
    pub safe_values: BTreeMap<AttributeKey, AttributeValue>,
    /// Present for schema-v2 snapshots embedded in shows. The complete profile snapshot insulates
    /// an already-patched show from later desk-library revisions.
    #[serde(default)]
    pub profile_id: Option<FixtureId>,
    #[serde(default)]
    pub mode_id: Option<Uuid>,
    #[serde(default)]
    pub profile_snapshot: Option<Box<FixtureProfile>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FixturePhysicalProperties {
    pub pan_range_degrees: Option<f32>,
    pub tilt_range_degrees: Option<f32>,
    pub width_millimetres: Option<f32>,
    pub height_millimetres: Option<f32>,
    pub depth_millimetres: Option<f32>,
    pub weight_kilograms: Option<f32>,
    #[serde(default)]
    pub power_watts: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalLossPolicy {
    #[default]
    HoldLast,
    FadeToSafe {
        duration_millis: u64,
    },
    ImmediateSafe,
}

pub(crate) fn identity_matrix() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}
