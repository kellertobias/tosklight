use super::{ControlAction, FixtureChannel, GeometryGraph, HeadColorSystem};
use crate::{DirectControlProtocol, SignalLossPolicy};
use light_core::FixtureId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const FIXTURE_PROFILE_SCHEMA_VERSION: u16 = 2;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchPolicy {
    #[default]
    Dmx,
    VisualOnly,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnits {
    #[default]
    Auto,
    Metres,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureProfile {
    pub schema_version: u16,
    pub id: FixtureId,
    pub revision: u32,
    pub manufacturer: String,
    pub name: String,
    pub short_name: String,
    pub fixture_type: String,
    #[serde(default)]
    pub patch_policy: PatchPolicy,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub photograph_asset: Option<String>,
    #[serde(default)]
    pub stage_icon_asset: Option<String>,
    #[serde(default)]
    pub model_asset: Option<String>,
    #[serde(default)]
    pub model_units: ModelUnits,
    #[serde(default)]
    pub physical: ProfilePhysicalProperties,
    pub modes: Vec<FixtureMode>,
    #[serde(default)]
    pub hazardous: bool,
    #[serde(default)]
    pub direct_control_protocols: Vec<DirectControlProtocol>,
    #[serde(default)]
    pub signal_loss_policy: SignalLossPolicy,
    #[serde(default)]
    pub reserved_source: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfilePhysicalProperties {
    #[serde(default)]
    pub width_millimetres: Option<f32>,
    #[serde(default)]
    pub height_millimetres: Option<f32>,
    #[serde(default)]
    pub depth_millimetres: Option<f32>,
    #[serde(default)]
    pub weight_kilograms: Option<f32>,
    #[serde(default)]
    pub power_watts: Option<f32>,
    #[serde(default)]
    pub connectors: String,
    #[serde(default)]
    pub light_source: String,
    #[serde(default)]
    pub color_temperature_kelvin: Option<f32>,
    #[serde(default)]
    pub color_rendering_index: Option<f32>,
    #[serde(default)]
    pub luminous_output_lumens: Option<f32>,
    #[serde(default)]
    pub lens: String,
    #[serde(default)]
    pub beam_angle_degrees: Option<f32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FixtureMode {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub notes: String,
    pub splits: Vec<FixtureSplit>,
    pub heads: Vec<FixtureHead>,
    #[serde(default)]
    pub channels: Vec<FixtureChannel>,
    #[serde(default)]
    pub color_systems: Vec<HeadColorSystem>,
    #[serde(default)]
    pub control_actions: Vec<ControlAction>,
    #[serde(default)]
    pub geometry: GeometryGraph,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FixtureSplit {
    pub number: u16,
    pub footprint: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureHead {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub master_shared: bool,
}
