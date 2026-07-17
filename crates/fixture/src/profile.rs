use super::{
    ByteOrder, Capability, ChannelComponent, ColorCalibration, DirectControlProtocol,
    FixtureDefinition, FixturePhysicalProperties, LogicalHead, Parameter, ParameterMetadata,
    SignalLossPolicy,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use thiserror::Error;
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
    pub width_millimetres: Option<f32>,
    pub height_millimetres: Option<f32>,
    pub depth_millimetres: Option<f32>,
    pub weight_kilograms: Option<f32>,
    pub power_watts: Option<f32>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelResolution {
    U8,
    U16,
    U24,
    U32,
}

impl ChannelResolution {
    pub const fn bytes(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U24 => 3,
            Self::U32 => 4,
        }
    }

    pub const fn max_raw(self) -> u32 {
        match self {
            Self::U8 => u8::MAX as u32,
            Self::U16 => u16::MAX as u32,
            Self::U24 => 0x00ff_ffff,
            Self::U32 => u32::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelBehavior {
    #[default]
    Controlled,
    Static,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureChannel {
    pub id: Uuid,
    pub head_id: Uuid,
    /// Independently patchable address block containing this physical channel.
    pub split: u16,
    pub attribute: AttributeKey,
    pub resolution: ChannelResolution,
    /// Explicit 1-based component slots after the derived primary slot.
    #[serde(default)]
    pub secondary_slots: Vec<u16>,
    pub default_raw: u32,
    pub highlight_raw: u32,
    #[serde(default)]
    pub physical_min: Option<f32>,
    #[serde(default)]
    pub physical_max: Option<f32>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub snap: bool,
    #[serde(default)]
    pub reacts_to_virtual_intensity: bool,
    #[serde(default)]
    pub reacts_to_sequence_master: bool,
    #[serde(default)]
    pub reacts_to_group_master: bool,
    #[serde(default)]
    pub reacts_to_grand_master: bool,
    #[serde(default)]
    pub behavior: ChannelBehavior,
    #[serde(default)]
    pub functions: Vec<ChannelFunction>,
}

#[derive(Deserialize)]
struct FixtureModeCanonical {
    id: Uuid,
    name: String,
    #[serde(default)]
    notes: String,
    splits: Vec<FixtureSplit>,
    heads: Vec<FixtureHead>,
    #[serde(default)]
    channels: Vec<FixtureChannel>,
    #[serde(default)]
    color_systems: Vec<HeadColorSystem>,
    #[serde(default)]
    control_actions: Vec<ControlAction>,
    #[serde(default)]
    geometry: GeometryGraph,
}

impl<'de> Deserialize<'de> for FixtureMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let object = value.as_object_mut().ok_or_else(|| {
            <D::Error as serde::de::Error>::custom("fixture mode must be an object")
        })?;
        let legacy_head_splits = object
            .get("heads")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|head| {
                Some((
                    head.get("id")?.as_str()?.to_owned(),
                    head.get("split")?.as_u64()?,
                ))
            })
            .collect::<HashMap<_, _>>();
        if let Some(channels) = object
            .get_mut("channels")
            .and_then(serde_json::Value::as_array_mut)
        {
            for channel in channels {
                let Some(channel) = channel.as_object_mut() else {
                    continue;
                };
                if channel.contains_key("split") {
                    continue;
                }
                let split = channel
                    .get("head_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|head_id| legacy_head_splits.get(head_id))
                    .copied()
                    .ok_or_else(|| {
                        <D::Error as serde::de::Error>::custom(
                            "channel without split does not reference a legacy head split",
                        )
                    })?;
                channel.insert("split".into(), serde_json::Value::from(split));
            }
        }
        let canonical: FixtureModeCanonical = serde_json::from_value(value)
            .map_err(<D::Error as serde::de::Error>::custom)?;
        Ok(Self {
            id: canonical.id,
            name: canonical.name,
            notes: canonical.notes,
            splits: canonical.splits,
            heads: canonical.heads,
            channels: canonical.channels,
            color_systems: canonical.color_systems,
            control_actions: canonical.control_actions,
            geometry: canonical.geometry,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelScales {
    pub virtual_intensity: f32,
    pub sequence_master: f32,
    pub group_master: f32,
    pub grand_master: f32,
}

impl Default for ChannelScales {
    fn default() -> Self {
        Self {
            virtual_intensity: 1.0,
            sequence_master: 1.0,
            group_master: 1.0,
            grand_master: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelFunction {
    pub id: Uuid,
    pub name: String,
    pub dmx_from: u32,
    pub dmx_to: u32,
    pub attribute: AttributeKey,
    pub priority: i16,
    pub behavior: ChannelFunctionBehavior,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChannelFunctionBehavior {
    Continuous {
        physical_min: f32,
        physical_max: f32,
        unit: Option<String>,
    },
    Fixed {
        semantic_id: String,
        label: String,
        raw_value: u32,
    },
    Indexed {
        semantic_id: String,
        label: String,
        raw_value: u32,
    },
    Control {
        action_id: Uuid,
    },
}

impl ChannelFunction {
    pub fn continuous(name: impl Into<String>, attribute: AttributeKey, max_raw: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            dmx_from: 0,
            dmx_to: max_raw,
            attribute,
            priority: 0,
            behavior: ChannelFunctionBehavior::Continuous {
                physical_min: 0.0,
                physical_max: 1.0,
                unit: None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlActionKind {
    Latched,
    Momentary,
    TimedPulse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlAction {
    pub id: Uuid,
    pub name: String,
    pub kind: ControlActionKind,
    #[serde(default)]
    pub duration_millis: Option<u64>,
    pub assignments: Vec<ControlActionAssignment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlActionAssignment {
    pub channel_id: Uuid,
    pub active_raw: u32,
    pub inactive_raw: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeadColorSystem {
    pub head_id: Uuid,
    #[serde(default = "identity_color_correction")]
    pub correction_matrix: [[f32; 3]; 3],
    pub system: ColorSystem,
}

fn identity_color_correction() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ColorSystem {
    Additive {
        emitters: Vec<EmitterBinding>,
    },
    Subtractive {
        cyan_channel_id: Uuid,
        magenta_channel_id: Uuid,
        yellow_channel_id: Uuid,
    },
    DiscreteWheel {
        channel_id: Uuid,
        slots: Vec<ColorWheelSlot>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmitterBinding {
    pub channel_id: Uuid,
    pub name: String,
    pub xyz: Xyz,
    pub maximum_level: f32,
    #[serde(default = "default_response_curve")]
    pub response_curve: f32,
    #[serde(default)]
    pub visible: bool,
}

fn default_response_curve() -> f32 {
    1.0
}

fn valid_measured_xyz(xyz: Xyz) -> bool {
    [xyz.x, xyz.y, xyz.z]
        .into_iter()
        .all(|component| component.is_finite() && component >= 0.0)
}

fn legacy_emitter_is_visible(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    !matches!(name.as_str(), "uv" | "ir")
        && !name.contains("ultraviolet")
        && !name.contains("infrared")
}

const SEMANTIC_WHITE_XYZ: Xyz = Xyz {
    x: 0.950_47,
    y: 1.0,
    z: 1.088_83,
};

fn semantic_endpoint(max: u32, full: bool, invert: bool) -> u32 {
    if full != invert { max } else { 0 }
}

fn identifies_open_or_white(value: &str) -> bool {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>();
    let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    matches!(
        normalized.as_str(),
        "open"
            | "white"
            | "clear"
            | "color open"
            | "colour open"
            | "color white"
            | "colour white"
            | "open white"
            | "no color"
            | "no colour"
    )
}

fn semantic_highlight_raw(
    attribute: &AttributeKey,
    resolution: ChannelResolution,
    default_raw: u32,
    invert: bool,
    capabilities: &[Capability],
) -> u32 {
    let max = resolution.max_raw();
    if attribute.is_intensity() {
        return semantic_endpoint(max, true, invert);
    }
    match attribute.0.as_str() {
        "color.red" | "color.green" | "color.blue" | "color.white" | "color.cold_white"
        | "color.warm_white" => {
            return semantic_endpoint(max, true, invert);
        }
        "color.cyan" | "color.magenta" | "color.yellow" => {
            return semantic_endpoint(max, false, invert);
        }
        _ => {}
    }
    if let Some(emitter) = attribute.0.strip_prefix("color.emitter.")
        && matches!(
            emitter,
            "red" | "green" | "blue" | "white" | "cold_white" | "warm_white"
        )
    {
        return semantic_endpoint(max, true, invert);
    }
    if attribute.0.starts_with("color.wheel")
        && let Some(capability) = capabilities
            .iter()
            .find(|capability| identifies_open_or_white(&capability.name))
    {
        let midpoint = u32::from(capability.dmx_from)
            + u32::from(capability.dmx_to.saturating_sub(capability.dmx_from)) / 2;
        return ((u64::from(midpoint) * u64::from(max) + 127) / 255) as u32;
    }
    default_raw
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorWheelSlot {
    pub semantic_id: String,
    pub label: String,
    pub dmx_from: u32,
    pub dmx_to: u32,
    #[serde(default)]
    pub measured_xyz: Option<Xyz>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GeometryGraph {
    #[serde(default)]
    pub nodes: Vec<GeometryNode>,
    #[serde(default)]
    pub emitters: Vec<GeometryEmitter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeometryNode {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub transform: Transform3,
    #[serde(default)]
    pub pivot: Vector3,
    #[serde(default)]
    pub glb_node: Option<String>,
    #[serde(default)]
    pub motion: Option<GeometryMotion>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Transform3 {
    pub translation: Vector3,
    pub rotation_degrees: Vector3,
    pub scale: Vector3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeometryMotion {
    pub attribute: AttributeKey,
    pub kind: GeometryMotionKind,
    pub axis: Vector3,
    pub physical_min: f32,
    pub physical_max: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryMotionKind {
    Rotation,
    Translation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeometryEmitter {
    pub id: Uuid,
    pub name: String,
    pub node_id: Uuid,
    pub head_id: Uuid,
    #[serde(default)]
    pub origin: Vector3,
    #[serde(default)]
    pub orientation_degrees: Vector3,
    pub beam_angle_degrees: f32,
    pub field_angle_degrees: f32,
    #[serde(default)]
    pub feather: f32,
    #[serde(default)]
    pub focus: f32,
    pub layout: EmitterLayout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EmitterLayout {
    Point,
    Matrix {
        columns: u16,
        rows: u16,
        spacing: Vector3,
    },
    Ring {
        count: u16,
        radius_millimetres: f32,
    },
    Strip {
        count: u16,
        spacing_millimetres: f32,
    },
    ExplicitPixels {
        positions: Vec<Vector3>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryTemplate {
    Fixed,
    MovingHead,
    Bar,
    Matrix,
    SharedPanMultiHead,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ProfileError {
    #[error("invalid fixture profile: {0}")]
    Invalid(String),
}

impl FixtureProfile {
    pub fn blank() -> Self {
        let profile_id = FixtureId::new();
        let mode_id = Uuid::new_v4();
        let head_id = Uuid::new_v4();
        Self {
            schema_version: FIXTURE_PROFILE_SCHEMA_VERSION,
            id: profile_id,
            revision: 0,
            manufacturer: String::new(),
            name: String::new(),
            short_name: String::new(),
            fixture_type: "other".into(),
            patch_policy: PatchPolicy::Dmx,
            notes: String::new(),
            photograph_asset: None,
            stage_icon_asset: None,
            model_asset: None,
            model_units: ModelUnits::Auto,
            physical: ProfilePhysicalProperties::default(),
            modes: vec![FixtureMode {
                id: mode_id,
                name: "Default".into(),
                notes: String::new(),
                splits: vec![FixtureSplit {
                    number: 1,
                    footprint: 1,
                }],
                heads: vec![FixtureHead {
                    id: head_id,
                    name: "Main".into(),
                    master_shared: true,
                }],
                channels: Vec::new(),
                color_systems: Vec::new(),
                control_actions: Vec::new(),
                geometry: GeometryGraph::template(GeometryTemplate::Fixed, &[head_id]),
            }],
            hazardous: false,
            direct_control_protocols: Vec::new(),
            signal_loss_policy: SignalLossPolicy::HoldLast,
            reserved_source: None,
        }
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.schema_version != FIXTURE_PROFILE_SCHEMA_VERSION {
            return Err(ProfileError::Invalid("unsupported schema version".into()));
        }
        if self.manufacturer.trim().is_empty() || self.name.trim().is_empty() {
            return Err(ProfileError::Invalid(
                "manufacturer and fixture name are required".into(),
            ));
        }
        if self.modes.is_empty() {
            return Err(ProfileError::Invalid(
                "at least one mode is required".into(),
            ));
        }
        validate_positive("width", self.physical.width_millimetres)?;
        validate_positive("height", self.physical.height_millimetres)?;
        validate_positive("depth", self.physical.depth_millimetres)?;
        validate_positive("weight", self.physical.weight_kilograms)?;
        validate_positive("power", self.physical.power_watts)?;
        let mut mode_ids = HashSet::new();
        for mode in &self.modes {
            if !mode_ids.insert(mode.id) {
                return Err(ProfileError::Invalid("mode IDs must be unique".into()));
            }
            mode.validate_for_patch_policy(self.patch_policy)?;
        }
        Ok(())
    }

    pub fn mode(&self, id: Uuid) -> Option<&FixtureMode> {
        self.modes.iter().find(|mode| mode.id == id)
    }

    pub fn resolved_definition(&self, mode_id: Uuid) -> Result<FixtureDefinition, ProfileError> {
        self.validate()?;
        let mode = self
            .mode(mode_id)
            .ok_or_else(|| ProfileError::Invalid("mode does not exist".into()))?;
        let primary = mode.primary_slots()?;
        let head_indices = mode
            .heads
            .iter()
            .enumerate()
            .map(|(index, head)| (head.id, index as u16))
            .collect::<HashMap<_, _>>();
        let mut heads = Vec::new();
        for (index, head) in mode.heads.iter().enumerate() {
            let parameters = mode
                .channels
                .iter()
                .filter(|channel| channel.head_id == head.id)
                .map(|channel| {
                    let mut slots = vec![*primary.get(&channel.id).expect("validated primary")];
                    slots.extend(channel.secondary_slots.iter().copied());
                    let max = channel.resolution.max_raw();
                    Parameter {
                        attribute: channel.attribute.clone(),
                        components: slots
                            .into_iter()
                            .map(|slot| ChannelComponent {
                                offset: slot - 1,
                                byte_order: ByteOrder::MsbFirst,
                            })
                            .collect(),
                        default: channel.default_raw as f32 / max as f32,
                        virtual_dimmer: channel.reacts_to_virtual_intensity,
                        metadata: ParameterMetadata {
                            physical_min: channel.physical_min.unwrap_or(0.0),
                            physical_max: channel.physical_max.unwrap_or(1.0),
                            unit: channel.unit.clone(),
                            invert: channel.invert,
                            ..Default::default()
                        },
                        capabilities: Vec::new(),
                    }
                })
                .collect();
            heads.push(LogicalHead {
                index: index as u16,
                name: head.name.clone(),
                shared: head.master_shared,
                parameters,
            });
        }
        let color_calibration = mode.color_systems.iter().find_map(|system| {
            let ColorSystem::Additive { emitters } = &system.system else {
                return None;
            };
            Some(ColorCalibration {
                emitters: emitters
                    .iter()
                    .map(|emitter| super::EmitterCalibration {
                        name: emitter.name.clone(),
                        xyz: emitter.xyz,
                        limit: emitter.maximum_level,
                    })
                    .collect(),
                correction_matrix: system.correction_matrix,
            })
        });
        let footprint = mode
            .splits
            .iter()
            .find(|split| split.number == 1)
            .or_else(|| mode.splits.first())
            .map(|split| split.footprint)
            .unwrap_or(1);
        let _ = head_indices;
        Ok(FixtureDefinition {
            schema_version: FIXTURE_PROFILE_SCHEMA_VERSION,
            id: self.id,
            revision: self.revision,
            manufacturer: self.manufacturer.clone(),
            device_type: self.fixture_type.clone(),
            name: self.name.clone(),
            model: self.short_name.clone(),
            mode: mode.name.clone(),
            footprint,
            heads,
            color_calibration,
            physical: FixturePhysicalProperties {
                width_millimetres: self.physical.width_millimetres,
                height_millimetres: self.physical.height_millimetres,
                depth_millimetres: self.physical.depth_millimetres,
                weight_kilograms: self.physical.weight_kilograms,
                power_watts: self.physical.power_watts,
                ..Default::default()
            },
            model_asset: self.model_asset.clone(),
            icon_asset: self.stage_icon_asset.clone(),
            hazardous: self.hazardous,
            direct_control_protocols: self.direct_control_protocols.clone(),
            signal_loss_policy: self.signal_loss_policy,
            safe_values: BTreeMap::new(),
            profile_id: Some(self.id),
            mode_id: Some(mode.id),
            profile_snapshot: Some(Box::new(self.clone())),
        })
    }

    pub fn from_legacy_modes(definitions: &[FixtureDefinition]) -> Result<Self, ProfileError> {
        let first = definitions
            .first()
            .ok_or_else(|| ProfileError::Invalid("legacy mode list is empty".into()))?;
        if definitions.iter().any(|definition| {
            !definition
                .manufacturer
                .eq_ignore_ascii_case(&first.manufacturer)
                || !definition.model.eq_ignore_ascii_case(&first.model)
                || definition.device_type != first.device_type
                || definition.physical.width_millimetres != first.physical.width_millimetres
                || definition.physical.height_millimetres != first.physical.height_millimetres
                || definition.physical.depth_millimetres != first.physical.depth_millimetres
                || definition.physical.weight_kilograms != first.physical.weight_kilograms
        }) {
            return Err(ProfileError::Invalid(
                "legacy modes have conflicting fixture-level metadata".into(),
            ));
        }
        // Reuse one legacy definition identity so migration remains deterministic even when two
        // records share a manufacturer/model label but have conflicting fixture-level metadata.
        let profile_id = first.id;
        let modes = definitions
            .iter()
            .map(FixtureMode::from_legacy)
            .collect::<Result<Vec<_>, _>>()?;
        let profile = Self {
            schema_version: FIXTURE_PROFILE_SCHEMA_VERSION,
            id: profile_id,
            revision: 1,
            manufacturer: first.manufacturer.clone(),
            name: first.display_name().to_owned(),
            short_name: first.model.clone(),
            fixture_type: first.device_type.clone(),
            patch_policy: PatchPolicy::Dmx,
            notes: String::new(),
            photograph_asset: None,
            stage_icon_asset: first.icon_asset.clone(),
            model_asset: first.model_asset.clone(),
            model_units: ModelUnits::Auto,
            physical: ProfilePhysicalProperties {
                width_millimetres: first.physical.width_millimetres,
                height_millimetres: first.physical.height_millimetres,
                depth_millimetres: first.physical.depth_millimetres,
                weight_kilograms: first.physical.weight_kilograms,
                power_watts: first.physical.power_watts,
            },
            modes,
            hazardous: first.hazardous,
            direct_control_protocols: first.direct_control_protocols.clone(),
            signal_loss_policy: first.signal_loss_policy,
            reserved_source: None,
        };
        profile.validate()?;
        Ok(profile)
    }
}

impl FixtureMode {
    pub fn validate(&self) -> Result<(), ProfileError> {
        self.validate_for_patch_policy(PatchPolicy::Dmx)
    }

    fn validate_for_patch_policy(&self, patch_policy: PatchPolicy) -> Result<(), ProfileError> {
        if self.name.trim().is_empty() {
            return Err(ProfileError::Invalid("mode name is required".into()));
        }
        if self.splits.is_empty() || self.heads.is_empty() {
            return Err(ProfileError::Invalid(
                "a mode needs a split and a head".into(),
            ));
        }
        let split_map = self
            .splits
            .iter()
            .map(|split| (split.number, split.footprint))
            .collect::<BTreeMap<_, _>>();
        let invalid_split = self.splits.iter().any(|split| {
            split.number == 0
                || match patch_policy {
                    PatchPolicy::Dmx => !(1..=512).contains(&split.footprint),
                    PatchPolicy::VisualOnly => split.footprint != 0,
                }
        });
        if split_map.len() != self.splits.len() || invalid_split {
            return Err(ProfileError::Invalid(
                match patch_policy {
                    PatchPolicy::Dmx => "split numbers must be unique and footprints must be 1-512",
                    PatchPolicy::VisualOnly => {
                        "visual-only split numbers must be unique and footprints must be zero"
                    }
                }
                .into(),
            ));
        }
        if patch_policy == PatchPolicy::VisualOnly
            && (!self.channels.is_empty()
                || !self.color_systems.is_empty()
                || !self.control_actions.is_empty())
        {
            return Err(ProfileError::Invalid(
                "visual-only modes cannot define DMX channels, color systems, or control actions"
                    .into(),
            ));
        }
        let mut head_ids = HashSet::new();
        let mut masters = 0;
        for head in &self.heads {
            if head.name.trim().is_empty() || !head_ids.insert(head.id) {
                return Err(ProfileError::Invalid(
                    "head names and IDs must be unique".into(),
                ));
            }
            masters += usize::from(head.master_shared);
        }
        if masters > 1 {
            return Err(ProfileError::Invalid(
                "at most one head can be master/shared".into(),
            ));
        }
        let mut channel_ids = HashSet::new();
        for channel in &self.channels {
            if !channel_ids.insert(channel.id) || !head_ids.contains(&channel.head_id) {
                return Err(ProfileError::Invalid(
                    "channel IDs must be unique and reference an existing head".into(),
                ));
            }
            if !split_map.contains_key(&channel.split) {
                return Err(ProfileError::Invalid(
                    "channel references a missing split".into(),
                ));
            }
            channel.validate()?;
        }
        self.primary_slots()?;
        for head in &self.heads {
            if !head_ids.contains(&head.id) {
                return Err(ProfileError::Invalid("invalid head".into()));
            }
        }
        let action_ids = self
            .control_actions
            .iter()
            .map(|action| action.id)
            .collect::<HashSet<_>>();
        if action_ids.len() != self.control_actions.len() {
            return Err(ProfileError::Invalid(
                "control action IDs must be unique".into(),
            ));
        }
        for action in &self.control_actions {
            if action.assignments.is_empty() {
                return Err(ProfileError::Invalid(
                    "control actions need assignments".into(),
                ));
            }
            if action.kind == ControlActionKind::TimedPulse
                && action.duration_millis.is_none_or(|duration| duration == 0)
            {
                return Err(ProfileError::Invalid(
                    "timed pulse actions need a positive duration".into(),
                ));
            }
            for assignment in &action.assignments {
                let channel = self
                    .channels
                    .iter()
                    .find(|channel| channel.id == assignment.channel_id)
                    .ok_or_else(|| {
                        ProfileError::Invalid("action references a missing channel".into())
                    })?;
                if assignment.active_raw > channel.resolution.max_raw()
                    || assignment.inactive_raw > channel.resolution.max_raw()
                {
                    return Err(ProfileError::Invalid(
                        "action raw value is out of range".into(),
                    ));
                }
            }
        }
        for channel in &self.channels {
            for function in &channel.functions {
                if let ChannelFunctionBehavior::Control { action_id } = function.behavior
                    && !action_ids.contains(&action_id)
                {
                    return Err(ProfileError::Invalid(
                        "channel function references a missing control action".into(),
                    ));
                }
            }
        }
        self.validate_color_systems(&head_ids, &channel_ids)?;
        self.geometry.validate(&head_ids)?;
        Ok(())
    }

    pub fn primary_slots(&self) -> Result<HashMap<Uuid, u16>, ProfileError> {
        let footprint = self
            .splits
            .iter()
            .map(|split| (split.number, split.footprint))
            .collect::<HashMap<_, _>>();
        let mut reserved = HashMap::<u16, BTreeSet<u16>>::new();
        for channel in &self.channels {
            let split = channel.split;
            if channel.secondary_slots.len() + 1 != channel.resolution.bytes() {
                return Err(ProfileError::Invalid(format!(
                    "{}-bit channels require {} secondary slots",
                    channel.resolution.bytes() * 8,
                    channel.resolution.bytes() - 1
                )));
            }
            let limit = *footprint
                .get(&split)
                .ok_or_else(|| ProfileError::Invalid("channel references a missing split".into()))?;
            for slot in &channel.secondary_slots {
                if *slot == 0 || *slot > limit || !reserved.entry(split).or_default().insert(*slot)
                {
                    return Err(ProfileError::Invalid(
                        "component slots are duplicated or outside the split footprint".into(),
                    ));
                }
            }
        }
        let mut next = HashMap::<u16, u16>::new();
        let mut used = reserved.clone();
        let mut result = HashMap::new();
        for channel in &self.channels {
            let split = channel.split;
            let limit = footprint[&split];
            let cursor = next.entry(split).or_insert(1);
            while used.get(&split).is_some_and(|slots| slots.contains(cursor)) {
                *cursor += 1;
            }
            if *cursor > limit {
                return Err(ProfileError::Invalid(
                    "channel rows exceed the split footprint".into(),
                ));
            }
            used.entry(split).or_default().insert(*cursor);
            result.insert(channel.id, *cursor);
            *cursor += 1;
        }
        Ok(result)
    }

    /// Internal semantic address used by the Programmer for an atomic control-action assignment.
    /// It is channel-specific because one action may drive several channels that otherwise expose
    /// the same public attribute.
    pub fn control_action_attribute(channel_id: Uuid) -> AttributeKey {
        AttributeKey(format!("__fixture_control_channel.{channel_id}"))
    }

    /// Resolve one physical channel after normal semantic LTP/HTP resolution. Competing
    /// functions claim the channel only when their exact semantic address is explicitly present;
    /// the highest configured priority wins and release reveals the next eligible function.
    pub fn resolve_channel_raw(
        &self,
        channel: &FixtureChannel,
        values: &HashMap<AttributeKey, AttributeValue>,
        highlighted: bool,
        highlight_override: Option<u32>,
        scales: ChannelScales,
    ) -> u32 {
        let max = channel.resolution.max_raw();
        let resolved = if highlighted {
            ResolvedChannelRaw::Exact(highlight_override.unwrap_or(channel.highlight_raw))
        } else if channel.behavior == ChannelBehavior::Static {
            ResolvedChannelRaw::Exact(channel.default_raw)
        } else if let Some(AttributeValue::RawDmxExact(value)) =
            values.get(&Self::control_action_attribute(channel.id))
        {
            ResolvedChannelRaw::Exact(*value)
        } else {
            channel
                .functions
                .iter()
                .enumerate()
                .filter_map(|(index, function)| {
                    function_value(function, values)
                        .map(|raw| (function.priority, std::cmp::Reverse(index), raw))
                })
                .max_by_key(|(priority, order, _)| (*priority, *order))
                .map(|(_, _, raw)| raw)
                .or_else(|| {
                    values
                        .get(&channel.attribute)
                        .and_then(|value| mapped_raw(value, 0, max))
                })
                .unwrap_or(ResolvedChannelRaw::Exact(channel.default_raw))
        };
        let mut scale = 1.0_f64;
        if !highlighted {
            if channel.reacts_to_virtual_intensity {
                scale *= f64::from(scales.virtual_intensity.clamp(0.0, 1.0));
            }
            if channel.reacts_to_sequence_master {
                scale *= f64::from(scales.sequence_master.clamp(0.0, 1.0));
            }
            if channel.reacts_to_group_master {
                scale *= f64::from(scales.group_master.clamp(0.0, 1.0));
            }
        }
        // Grand Master is the only ordinary master above transient Highlight. Blackout and
        // hazardous safe values are enforced by the engine after this channel resolution.
        if channel.reacts_to_grand_master {
            scale *= f64::from(scales.grand_master.clamp(0.0, 1.0));
        }
        match resolved {
            ResolvedChannelRaw::Semantic { raw, from, to } => {
                let from = from.min(max);
                let to = to.min(max).max(from);
                let raw = raw.clamp(from, to);
                let scaled = (f64::from(raw - from) * scale)
                    .round()
                    .clamp(0.0, f64::from(to - from)) as u32;
                if channel.invert {
                    to.saturating_sub(scaled)
                } else {
                    from.saturating_add(scaled)
                }
            }
            ResolvedChannelRaw::Exact(raw) => {
                let raw = raw.min(max);
                if channel.invert {
                    max.saturating_sub(
                        (f64::from(max - raw) * scale)
                            .round()
                            .clamp(0.0, f64::from(max)) as u32,
                    )
                } else {
                    (f64::from(raw) * scale).round().clamp(0.0, f64::from(max)) as u32
                }
            }
        }
    }

    /// Returns the semantic attribute that currently owns a physical channel. Defaults and Static
    /// behavior deliberately return `None`: neither represents an explicitly active source and
    /// therefore neither should acquire a source-specific sequence-master scale.
    pub fn active_attribute_for_channel<'a>(
        &'a self,
        channel: &'a FixtureChannel,
        values: &'a HashMap<AttributeKey, AttributeValue>,
    ) -> Option<&'a AttributeKey> {
        if channel.behavior == ChannelBehavior::Static {
            return None;
        }
        values
            .get_key_value(&Self::control_action_attribute(channel.id))
            .map(|(attribute, _)| attribute)
            .or_else(|| {
                channel
                    .functions
                    .iter()
                    .enumerate()
                    .filter(|(_, function)| function_value(function, values).is_some())
                    .max_by_key(|(index, function)| (function.priority, std::cmp::Reverse(*index)))
                    .map(|(_, function)| &function.attribute)
            })
            .or_else(|| {
                values
                    .contains_key(&channel.attribute)
                    .then_some(&channel.attribute)
            })
    }

    /// Whether a semantic address belonging to one logical head must bypass fades. A channel can
    /// claim an attribute either directly or through one of its multi-function ranges.
    pub fn head_attribute_is_snap(&self, head_id: Uuid, attribute: &AttributeKey) -> bool {
        self.channels.iter().any(|channel| {
            channel.head_id == head_id
                && channel.snap
                && (channel.attribute == *attribute
                    || channel
                        .functions
                        .iter()
                        .any(|function| function.attribute == *attribute))
        })
    }

    pub fn encode_channel(
        &self,
        frame: &mut [u8; 512],
        base: u16,
        channel: &FixtureChannel,
        raw: u32,
    ) -> Result<(), ProfileError> {
        let primary = self.primary_slots()?[&channel.id];
        let mut slots = vec![primary];
        slots.extend(channel.secondary_slots.iter().copied());
        if slots.len() != channel.resolution.bytes() {
            return Err(ProfileError::Invalid(
                "channel component count is invalid".into(),
            ));
        }
        for (index, slot) in slots.into_iter().enumerate() {
            let absolute =
                usize::from(base.saturating_sub(1)) + usize::from(slot.saturating_sub(1));
            if base == 0 || absolute >= frame.len() {
                return Err(ProfileError::Invalid(
                    "encoded channel exceeds its universe".into(),
                ));
            }
            let shift = 8 * (channel.resolution.bytes() - index - 1);
            frame[absolute] = ((raw >> shift) & 0xff) as u8;
        }
        Ok(())
    }

    /// Resolve an abstract XYZ color through the configured head system. Additive calibration uses
    /// bounded non-negative optimization; missing calibration falls back deterministically to RGB
    /// or CMY. UV/non-visible emitters are excluded unless directly programmed.
    pub fn resolve_color(
        &self,
        head_id: Uuid,
        target: Xyz,
    ) -> Result<HashMap<Uuid, u32>, ProfileError> {
        let Some(system) = self
            .color_systems
            .iter()
            .find(|system| system.head_id == head_id)
        else {
            return Ok(HashMap::new());
        };
        let mut output = HashMap::new();
        match &system.system {
            ColorSystem::Additive { emitters } => {
                let visible = emitters
                    .iter()
                    .filter(|emitter| emitter.visible)
                    .collect::<Vec<_>>();
                let levels = if visible.len() >= 3 {
                    let calibration = ColorCalibration {
                        emitters: visible
                            .iter()
                            .map(|emitter| super::EmitterCalibration {
                                name: emitter.name.clone(),
                                xyz: emitter.xyz,
                                // Optimization happens in emitted-light space. The configured
                                // maximum is a drive limit, so convert it through the response
                                // curve before constraining the optical solution.
                                limit: emitter.maximum_level.powf(emitter.response_curve),
                            })
                            .collect(),
                        correction_matrix: system.correction_matrix,
                    };
                    super::mix_color(target, &calibration)
                        .map_err(|error| ProfileError::Invalid(error.to_string()))?
                } else {
                    let rgb = xyz_to_srgb(target);
                    visible
                        .iter()
                        .map(|emitter| {
                            let name = emitter.name.to_ascii_lowercase();
                            if name.contains("red") {
                                rgb.0
                            } else if name.contains("green") {
                                rgb.1
                            } else if name.contains("blue") {
                                rgb.2
                            } else if name.contains("white") {
                                rgb.0.min(rgb.1).min(rgb.2)
                            } else {
                                0.0
                            }
                        })
                        .collect()
                };
                for (emitter, level) in visible.into_iter().zip(levels) {
                    let channel = self
                        .channels
                        .iter()
                        .find(|channel| channel.id == emitter.channel_id)
                        .ok_or_else(|| {
                            ProfileError::Invalid("emitter references a missing channel".into())
                        })?;
                    // The optimizer/fallback yields an emitted-light level. Apply the inverse
                    // response curve to obtain the deterministic DMX drive value, retaining the
                    // configured maximum drive as the final bound.
                    let drive = level
                        .clamp(0.0, 1.0)
                        .powf(1.0 / emitter.response_curve)
                        .clamp(0.0, emitter.maximum_level);
                    let max = channel.resolution.max_raw();
                    let raw = (drive * max as f32).round() as u32;
                    output.insert(
                        channel.id,
                        if channel.invert {
                            max.saturating_sub(raw)
                        } else {
                            raw
                        },
                    );
                }
            }
            ColorSystem::Subtractive {
                cyan_channel_id,
                magenta_channel_id,
                yellow_channel_id,
            } => {
                let (red, green, blue) = xyz_to_srgb(target);
                for (id, level) in [
                    (*cyan_channel_id, 1.0 - red),
                    (*magenta_channel_id, 1.0 - green),
                    (*yellow_channel_id, 1.0 - blue),
                ] {
                    let channel = self
                        .channels
                        .iter()
                        .find(|channel| channel.id == id)
                        .ok_or_else(|| {
                            ProfileError::Invalid("CMY system references a missing channel".into())
                        })?;
                    let max = channel.resolution.max_raw();
                    let raw = (level.clamp(0.0, 1.0) * max as f32).round() as u32;
                    output.insert(
                        id,
                        if channel.invert {
                            max.saturating_sub(raw)
                        } else {
                            raw
                        },
                    );
                }
            }
            ColorSystem::DiscreteWheel { channel_id, slots } => {
                if let Some(slot) = slots
                    .iter()
                    .filter_map(|slot| {
                        slot.measured_xyz
                            .map(|xyz| (slot, color_distance(target, xyz)))
                    })
                    .min_by(|left, right| left.1.total_cmp(&right.1))
                    .map(|(slot, _)| slot)
                {
                    output.insert(
                        *channel_id,
                        slot.dmx_from + (slot.dmx_to - slot.dmx_from) / 2,
                    );
                }
            }
        }
        Ok(output)
    }

    pub fn control_action_values(
        &self,
        action_id: Uuid,
        active: bool,
    ) -> Result<Vec<(Uuid, u32)>, ProfileError> {
        let action = self
            .control_actions
            .iter()
            .find(|action| action.id == action_id)
            .ok_or_else(|| ProfileError::Invalid("control action does not exist".into()))?;
        Ok(action
            .assignments
            .iter()
            .map(|assignment| {
                (
                    assignment.channel_id,
                    if active {
                        assignment.active_raw
                    } else {
                        assignment.inactive_raw
                    },
                )
            })
            .collect())
    }

    /// Apply the semantic full-and-white look only while deriving a new schema-v2 mode from a
    /// legacy definition. Authored schema-v2 Highlight raw values are never normalized on load or
    /// save, and per-fixture overrides live outside the profile entirely.
    fn apply_derived_highlight_defaults(&mut self) -> Result<(), ProfileError> {
        let mut color_values = HashMap::new();
        for system in &self.color_systems {
            match &system.system {
                ColorSystem::Additive { .. } | ColorSystem::Subtractive { .. } => {
                    color_values.extend(self.resolve_color(system.head_id, SEMANTIC_WHITE_XYZ)?);
                }
                ColorSystem::DiscreteWheel { channel_id, slots } => {
                    let slot = slots
                        .iter()
                        .find(|slot| {
                            identifies_open_or_white(&slot.semantic_id)
                                || identifies_open_or_white(&slot.label)
                        })
                        .or_else(|| {
                            slots
                                .iter()
                                .filter_map(|slot| {
                                    slot.measured_xyz
                                        .map(|xyz| (slot, color_distance(SEMANTIC_WHITE_XYZ, xyz)))
                                })
                                .min_by(|left, right| left.1.total_cmp(&right.1))
                                .map(|(slot, _)| slot)
                        });
                    if let Some(slot) = slot {
                        color_values.insert(
                            *channel_id,
                            slot.dmx_from + (slot.dmx_to - slot.dmx_from) / 2,
                        );
                    }
                }
            }
        }
        for channel in &mut self.channels {
            if let Some(raw) = color_values.get(&channel.id) {
                channel.highlight_raw = *raw;
            }
        }
        Ok(())
    }

    fn from_legacy(definition: &FixtureDefinition) -> Result<Self, ProfileError> {
        let mode_id = stable_uuid(&format!("{}\0{}", definition.id.0, definition.mode));
        let heads = definition
            .heads
            .iter()
            .map(|head| FixtureHead {
                id: stable_uuid(&format!("{mode_id}\0head\0{}", head.index)),
                name: head.name.clone(),
                master_shared: head.shared,
            })
            .collect::<Vec<_>>();
        let mut channels = Vec::new();
        for (head_index, legacy_head) in definition.heads.iter().enumerate() {
            let has_virtual_dimmer = legacy_head.parameters.iter().any(|parameter| {
                parameter.virtual_dimmer
                    && parameter.attribute.is_intensity()
                    && parameter.components.is_empty()
            });
            for (parameter_index, parameter) in legacy_head.parameters.iter().enumerate() {
                if parameter.components.is_empty() {
                    continue;
                }
                let resolution = match parameter.components.len() {
                    1 => ChannelResolution::U8,
                    2 => ChannelResolution::U16,
                    3 => ChannelResolution::U24,
                    4 => ChannelResolution::U32,
                    _ => {
                        return Err(ProfileError::Invalid(
                            "legacy channel resolution is invalid".into(),
                        ));
                    }
                };
                let max = resolution.max_raw();
                let attribute = parameter.attribute.clone();
                let default_raw = (parameter.default.clamp(0.0, 1.0) * max as f32).round() as u32;
                let highlight_raw = semantic_highlight_raw(
                    &attribute,
                    resolution,
                    default_raw,
                    parameter.metadata.invert,
                    &parameter.capabilities,
                );
                channels.push(FixtureChannel {
                    id: stable_uuid(&format!(
                        "{mode_id}\0channel\0{head_index}\0{parameter_index}\0{}",
                        attribute.0
                    )),
                    head_id: heads[head_index].id,
                    split: 1,
                    attribute: attribute.clone(),
                    resolution,
                    secondary_slots: parameter
                        .components
                        .iter()
                        .skip(1)
                        .map(|component| component.offset + 1)
                        .collect(),
                    default_raw,
                    highlight_raw,
                    physical_min: Some(parameter.metadata.physical_min),
                    physical_max: Some(parameter.metadata.physical_max),
                    unit: parameter.metadata.unit.clone(),
                    invert: parameter.metadata.invert,
                    snap: false,
                    reacts_to_virtual_intensity: parameter.virtual_dimmer
                        || (has_virtual_dimmer && attribute.0.starts_with("color.")),
                    reacts_to_sequence_master: attribute.is_intensity(),
                    reacts_to_group_master: attribute.is_intensity(),
                    reacts_to_grand_master: attribute.is_intensity(),
                    behavior: ChannelBehavior::Controlled,
                    functions: vec![ChannelFunction {
                        id: stable_uuid(&format!(
                            "{mode_id}\0function\0{head_index}\0{parameter_index}"
                        )),
                        name: attribute.0.clone(),
                        dmx_from: 0,
                        dmx_to: max,
                        attribute,
                        priority: 0,
                        behavior: ChannelFunctionBehavior::Continuous {
                            physical_min: parameter.metadata.physical_min,
                            physical_max: parameter.metadata.physical_max,
                            unit: parameter.metadata.unit.clone(),
                        },
                    }],
                });
            }
        }
        let color_systems = definition
            .color_calibration
            .as_ref()
            .map(|calibration| {
                heads
                    .iter()
                    .filter_map(|head| {
                        let emitters = calibration
                            .emitters
                            .iter()
                            .filter_map(|emitter| {
                                let attribute = AttributeKey(format!(
                                    "color.emitter.{}",
                                    emitter.name.to_lowercase()
                                ));
                                channels
                                    .iter()
                                    .find(|channel| {
                                        channel.head_id == head.id && channel.attribute == attribute
                                    })
                                    .map(|channel| EmitterBinding {
                                        channel_id: channel.id,
                                        name: emitter.name.clone(),
                                        xyz: emitter.xyz,
                                        maximum_level: emitter.limit,
                                        response_curve: 1.0,
                                        visible: legacy_emitter_is_visible(&emitter.name),
                                    })
                            })
                            .collect::<Vec<_>>();
                        (!emitters.is_empty()).then_some(HeadColorSystem {
                            head_id: head.id,
                            correction_matrix: calibration.correction_matrix,
                            system: ColorSystem::Additive { emitters },
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let geometry_head_ids = heads.iter().map(|head| head.id).collect::<Vec<_>>();
        let mut mode = Self {
            id: mode_id,
            name: definition.mode.clone(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: definition.footprint,
            }],
            heads,
            channels,
            color_systems,
            control_actions: Vec::new(),
            geometry: GeometryGraph::template(GeometryTemplate::Fixed, &geometry_head_ids),
        };
        mode.apply_derived_highlight_defaults()?;
        Ok(mode)
    }

    fn validate_color_systems(
        &self,
        head_ids: &HashSet<Uuid>,
        channel_ids: &HashSet<Uuid>,
    ) -> Result<(), ProfileError> {
        for system in &self.color_systems {
            if !head_ids.contains(&system.head_id) {
                return Err(ProfileError::Invalid(
                    "color system references a missing head".into(),
                ));
            }
            if system
                .correction_matrix
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
            {
                return Err(ProfileError::Invalid(
                    "color correction matrix is invalid".into(),
                ));
            }
            let references = match &system.system {
                ColorSystem::Additive { emitters } => {
                    if emitters.is_empty()
                        || emitters.iter().any(|emitter| {
                            !valid_measured_xyz(emitter.xyz)
                                || !emitter.maximum_level.is_finite()
                                || emitter.maximum_level <= 0.0
                                || emitter.maximum_level > 1.0
                                || !emitter.response_curve.is_finite()
                                || emitter.response_curve <= 0.0
                        })
                    {
                        return Err(ProfileError::Invalid(
                            "additive emitter calibration is invalid".into(),
                        ));
                    }
                    emitters
                        .iter()
                        .map(|emitter| emitter.channel_id)
                        .collect::<Vec<_>>()
                }
                ColorSystem::Subtractive {
                    cyan_channel_id,
                    magenta_channel_id,
                    yellow_channel_id,
                } => {
                    vec![*cyan_channel_id, *magenta_channel_id, *yellow_channel_id]
                }
                ColorSystem::DiscreteWheel { channel_id, slots } => {
                    let Some(channel) = self
                        .channels
                        .iter()
                        .find(|channel| channel.id == *channel_id)
                    else {
                        return Err(ProfileError::Invalid(
                            "color system references a missing channel".into(),
                        ));
                    };
                    let mut semantic_ids = HashSet::new();
                    if slots.is_empty()
                        || slots.iter().any(|slot| {
                            let semantic_id = slot.semantic_id.trim();
                            semantic_id.is_empty()
                                || !semantic_ids.insert(semantic_id)
                                || slot.label.trim().is_empty()
                                || slot.dmx_from > slot.dmx_to
                                || slot.dmx_to > channel.resolution.max_raw()
                                || slot
                                    .measured_xyz
                                    .is_some_and(|xyz| !valid_measured_xyz(xyz))
                        })
                    {
                        return Err(ProfileError::Invalid(
                            "color wheel slot metadata is invalid".into(),
                        ));
                    }
                    if slots
                        .windows(2)
                        .any(|pair| pair[0].dmx_to >= pair[1].dmx_from)
                    {
                        return Err(ProfileError::Invalid(
                            "color wheel slots must be sorted and non-overlapping".into(),
                        ));
                    }
                    vec![*channel_id]
                }
            };
            if references.iter().any(|id| !channel_ids.contains(id)) {
                return Err(ProfileError::Invalid(
                    "color system references a missing channel".into(),
                ));
            }
        }
        Ok(())
    }
}

impl FixtureChannel {
    pub fn validate(&self) -> Result<(), ProfileError> {
        let max = self.resolution.max_raw();
        if self.attribute.0.trim().is_empty() || self.default_raw > max || self.highlight_raw > max
        {
            return Err(ProfileError::Invalid(
                "channel attribute or raw values are invalid".into(),
            ));
        }
        if self
            .physical_min
            .zip(self.physical_max)
            .is_some_and(|(min, max)| !min.is_finite() || !max.is_finite() || min >= max)
        {
            return Err(ProfileError::Invalid(
                "channel physical range is invalid".into(),
            ));
        }
        let mut function_ids = HashSet::new();
        let mut ranges = self.functions.iter().collect::<Vec<_>>();
        ranges.sort_by_key(|function| function.dmx_from);
        for (index, function) in ranges.iter().enumerate() {
            if !function_ids.insert(function.id)
                || function.name.trim().is_empty()
                || function.attribute.0.trim().is_empty()
                || function.dmx_from > function.dmx_to
                || function.dmx_to > max
            {
                return Err(ProfileError::Invalid("channel function is invalid".into()));
            }
            if index > 0 && ranges[index - 1].dmx_to >= function.dmx_from {
                return Err(ProfileError::Invalid(
                    "channel function ranges overlap".into(),
                ));
            }
            match &function.behavior {
                ChannelFunctionBehavior::Continuous {
                    physical_min,
                    physical_max,
                    ..
                } if !physical_min.is_finite()
                    || !physical_max.is_finite()
                    || physical_min >= physical_max =>
                {
                    return Err(ProfileError::Invalid(
                        "continuous function range is invalid".into(),
                    ));
                }
                ChannelFunctionBehavior::Fixed { raw_value, .. }
                | ChannelFunctionBehavior::Indexed { raw_value, .. }
                    if *raw_value < function.dmx_from || *raw_value > function.dmx_to =>
                {
                    return Err(ProfileError::Invalid(
                        "fixed value is outside its function range".into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl GeometryGraph {
    pub fn template(template: GeometryTemplate, heads: &[Uuid]) -> Self {
        let root = stable_uuid(&format!("geometry-root-{template:?}"));
        let mut nodes = vec![GeometryNode {
            id: root,
            name: "Chassis".into(),
            parent_id: None,
            transform: Transform3 {
                scale: Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                ..Default::default()
            },
            pivot: Vector3::default(),
            glb_node: None,
            motion: None,
        }];
        match template {
            GeometryTemplate::MovingHead | GeometryTemplate::SharedPanMultiHead => {
                let pan = stable_uuid(&format!("{root}-pan"));
                nodes.push(GeometryNode {
                    id: pan,
                    name: "Pan arm".into(),
                    parent_id: Some(root),
                    transform: Transform3::default(),
                    pivot: Vector3::default(),
                    glb_node: None,
                    motion: Some(GeometryMotion {
                        attribute: AttributeKey("pan".into()),
                        kind: GeometryMotionKind::Rotation,
                        axis: Vector3 {
                            x: 0.0,
                            y: 1.0,
                            z: 0.0,
                        },
                        physical_min: -270.0,
                        physical_max: 270.0,
                    }),
                });
                for (index, _) in heads.iter().enumerate() {
                    nodes.push(GeometryNode {
                        id: stable_uuid(&format!("{pan}-tilt-{index}")),
                        name: format!("Tilt head {}", index + 1),
                        parent_id: Some(pan),
                        transform: Transform3::default(),
                        pivot: Vector3::default(),
                        glb_node: None,
                        motion: Some(GeometryMotion {
                            attribute: AttributeKey("tilt".into()),
                            kind: GeometryMotionKind::Rotation,
                            axis: Vector3 {
                                x: 1.0,
                                y: 0.0,
                                z: 0.0,
                            },
                            physical_min: -135.0,
                            physical_max: 135.0,
                        }),
                    });
                }
            }
            GeometryTemplate::Fixed | GeometryTemplate::Bar | GeometryTemplate::Matrix => {}
        }
        Self {
            nodes,
            emitters: Vec::new(),
        }
    }

    pub fn validate(&self, head_ids: &HashSet<Uuid>) -> Result<(), ProfileError> {
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<HashSet<_>>();
        if node_ids.len() != self.nodes.len() {
            return Err(ProfileError::Invalid(
                "geometry node IDs must be unique".into(),
            ));
        }
        for node in &self.nodes {
            if node
                .parent_id
                .is_some_and(|parent| !node_ids.contains(&parent) || parent == node.id)
            {
                return Err(ProfileError::Invalid("geometry parent is invalid".into()));
            }
            let mut seen = HashSet::new();
            let mut cursor = node.parent_id;
            while let Some(parent) = cursor {
                if !seen.insert(parent) {
                    return Err(ProfileError::Invalid(
                        "geometry hierarchy contains a cycle".into(),
                    ));
                }
                cursor = self
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id == parent)
                    .and_then(|candidate| candidate.parent_id);
            }
        }
        let mut emitter_ids = HashSet::new();
        for emitter in &self.emitters {
            if !emitter_ids.insert(emitter.id)
                || !node_ids.contains(&emitter.node_id)
                || !head_ids.contains(&emitter.head_id)
                || emitter.beam_angle_degrees < 0.0
                || emitter.field_angle_degrees < emitter.beam_angle_degrees
            {
                return Err(ProfileError::Invalid("geometry emitter is invalid".into()));
            }
        }
        Ok(())
    }

    pub fn resolved_transforms(
        &self,
        values: &HashMap<AttributeKey, AttributeValue>,
    ) -> HashMap<Uuid, Transform3> {
        self.nodes
            .iter()
            .map(|node| {
                let mut transform = node.transform;
                if let Some(motion) = &node.motion
                    && let Some(level) = values
                        .get(&motion.attribute)
                        .and_then(AttributeValue::normalized)
                {
                    let physical = motion.physical_min
                        + (motion.physical_max - motion.physical_min) * level.clamp(0.0, 1.0);
                    match motion.kind {
                        GeometryMotionKind::Rotation => {
                            transform.rotation_degrees.x += motion.axis.x * physical;
                            transform.rotation_degrees.y += motion.axis.y * physical;
                            transform.rotation_degrees.z += motion.axis.z * physical;
                        }
                        GeometryMotionKind::Translation => {
                            transform.translation.x += motion.axis.x * physical;
                            transform.translation.y += motion.axis.y * physical;
                            transform.translation.z += motion.axis.z * physical;
                        }
                    }
                }
                (node.id, transform)
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolvedChannelRaw {
    /// A semantic continuous value. Masters scale its distance from the function's zero endpoint,
    /// then inversion is applied inside that function range.
    Semantic { raw: u32, from: u32, to: u32 },
    /// A fixture-manual/raw value. Inversion does not reinterpret the value; opted-in masters
    /// instead move it toward the channel's configured physical-off endpoint.
    Exact(u32),
}

fn function_value(
    function: &ChannelFunction,
    values: &HashMap<AttributeKey, AttributeValue>,
) -> Option<ResolvedChannelRaw> {
    let value = values.get(&function.attribute)?;
    match (&function.behavior, value) {
        (ChannelFunctionBehavior::Continuous { .. }, value) => {
            mapped_raw(value, function.dmx_from, function.dmx_to)
        }
        (
            ChannelFunctionBehavior::Fixed {
                semantic_id,
                raw_value,
                ..
            },
            AttributeValue::Discrete(value),
        )
        | (
            ChannelFunctionBehavior::Indexed {
                semantic_id,
                raw_value,
                ..
            },
            AttributeValue::Discrete(value),
        ) if value == semantic_id => Some(ResolvedChannelRaw::Exact(*raw_value)),
        (ChannelFunctionBehavior::Control { action_id }, AttributeValue::Discrete(value))
            if value == &action_id.to_string() =>
        {
            Some(ResolvedChannelRaw::Exact(function.dmx_to))
        }
        _ => None,
    }
}

fn mapped_raw(value: &AttributeValue, from: u32, to: u32) -> Option<ResolvedChannelRaw> {
    match value {
        AttributeValue::Normalized(value) => Some(ResolvedChannelRaw::Semantic {
            raw: (f64::from(from) + f64::from(to - from) * f64::from(value.clamp(0.0, 1.0))).round()
                as u32,
            from,
            to,
        }),
        AttributeValue::RawDmx(value) => Some(ResolvedChannelRaw::Semantic {
            raw: from + ((u64::from(to - from) * u64::from(*value) + 127) / 255) as u32,
            from,
            to,
        }),
        // RawDmxExact is a physical channel value, not a semantic point in this function range.
        // Resolution clamping happens once in resolve_channel_raw.
        AttributeValue::RawDmxExact(value) => Some(ResolvedChannelRaw::Exact(*value)),
        _ => None,
    }
}

fn xyz_to_srgb(value: Xyz) -> (f32, f32, f32) {
    let linear = (
        3.240_454_2 * value.x - 1.537_138_5 * value.y - 0.498_531_4 * value.z,
        -0.969_266 * value.x + 1.876_010_8 * value.y + 0.041_556 * value.z,
        0.055_643_4 * value.x - 0.204_025_9 * value.y + 1.057_225_2 * value.z,
    );
    let encode = |value: f32| {
        let value = value.max(0.0);
        if value <= 0.003_130_8 {
            12.92 * value
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        }
    };
    (
        encode(linear.0).clamp(0.0, 1.0),
        encode(linear.1).clamp(0.0, 1.0),
        encode(linear.2).clamp(0.0, 1.0),
    )
}

fn color_distance(left: Xyz, right: Xyz) -> f32 {
    (left.x - right.x).powi(2) + (left.y - right.y).powi(2) + (left.z - right.z).powi(2)
}

fn validate_positive(name: &str, value: Option<f32>) -> Result<(), ProfileError> {
    if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        Err(ProfileError::Invalid(format!("{name} must be positive")))
    } else {
        Ok(())
    }
}

pub(crate) fn stable_uuid(value: &str) -> Uuid {
    fn hash(seed: u64, bytes: &[u8]) -> u64 {
        bytes.iter().fold(seed, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }
    let high = hash(0xcbf2_9ce4_8422_2325, value.as_bytes());
    let low = hash(0x8422_2325_cbf2_9ce4, value.as_bytes());
    Uuid::from_u128((u128::from(high) << 64) | u128::from(low))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(
        head_id: Uuid,
        resolution: ChannelResolution,
        secondary_slots: Vec<u16>,
    ) -> FixtureChannel {
        let max = resolution.max_raw();
        FixtureChannel {
            id: Uuid::new_v4(),
            head_id,
            split: 1,
            attribute: AttributeKey("intensity".into()),
            resolution,
            secondary_slots,
            default_raw: 0,
            highlight_raw: max,
            physical_min: Some(0.0),
            physical_max: Some(100.0),
            unit: Some("percent".into()),
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: true,
            reacts_to_group_master: true,
            reacts_to_grand_master: true,
            behavior: ChannelBehavior::Controlled,
            functions: vec![ChannelFunction::continuous(
                "Dimmer",
                AttributeKey("intensity".into()),
                max,
            )],
        }
    }

    fn additive_color_mode() -> FixtureMode {
        let mut profile = FixtureProfile::blank();
        let mode = &mut profile.modes[0];
        let head_id = mode.heads[0].id;
        let mut emitter = channel(head_id, ChannelResolution::U8, vec![]);
        emitter.attribute = AttributeKey("color.red".into());
        let channel_id = emitter.id;
        mode.channels = vec![emitter];
        mode.color_systems = vec![HeadColorSystem {
            head_id,
            correction_matrix: identity_color_correction(),
            system: ColorSystem::Additive {
                emitters: vec![EmitterBinding {
                    channel_id,
                    name: "Red".into(),
                    xyz: Xyz {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    maximum_level: 1.0,
                    response_curve: 1.0,
                    visible: true,
                }],
            },
        }];
        profile.modes.remove(0)
    }

    fn additive_emitter(mode: &mut FixtureMode) -> &mut EmitterBinding {
        let ColorSystem::Additive { emitters } = &mut mode.color_systems[0].system else {
            unreachable!("test mode is additive")
        };
        &mut emitters[0]
    }

    fn discrete_color_mode() -> FixtureMode {
        let mut profile = FixtureProfile::blank();
        let mode = &mut profile.modes[0];
        let head_id = mode.heads[0].id;
        let mut wheel = channel(head_id, ChannelResolution::U8, vec![]);
        wheel.attribute = AttributeKey("color.wheel.1".into());
        let channel_id = wheel.id;
        mode.channels = vec![wheel];
        mode.color_systems = vec![HeadColorSystem {
            head_id,
            correction_matrix: identity_color_correction(),
            system: ColorSystem::DiscreteWheel {
                channel_id,
                slots: vec![
                    ColorWheelSlot {
                        semantic_id: "red".into(),
                        label: "Red".into(),
                        dmx_from: 0,
                        dmx_to: 40,
                        measured_xyz: Some(Xyz {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0,
                        }),
                    },
                    ColorWheelSlot {
                        semantic_id: "blue".into(),
                        label: "Blue".into(),
                        dmx_from: 100,
                        dmx_to: 140,
                        measured_xyz: Some(Xyz {
                            x: 0.0,
                            y: 0.0,
                            z: 1.0,
                        }),
                    },
                ],
            },
        }];
        profile.modes.remove(0)
    }

    fn wheel_slots(mode: &mut FixtureMode) -> &mut Vec<ColorWheelSlot> {
        let ColorSystem::DiscreteWheel { slots, .. } = &mut mode.color_systems[0].system else {
            unreachable!("test mode is a discrete wheel")
        };
        slots
    }

    #[test]
    fn derives_primary_slots_around_reserved_component_bytes() {
        let head_id = Uuid::new_v4();
        let first = channel(head_id, ChannelResolution::U16, vec![2]);
        let second = channel(head_id, ChannelResolution::U24, vec![5, 6]);
        let third = channel(head_id, ChannelResolution::U8, vec![]);
        let mode = FixtureMode {
            id: Uuid::new_v4(),
            name: "Mode".into(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: 6,
            }],
            heads: vec![FixtureHead {
                id: head_id,
                name: "Main".into(),
                master_shared: true,
            }],
            channels: vec![first.clone(), second.clone(), third.clone()],
            color_systems: vec![],
            control_actions: vec![],
            geometry: GeometryGraph::default(),
        };
        let slots = mode.primary_slots().unwrap();
        assert_eq!(slots[&first.id], 1);
        assert_eq!(slots[&second.id], 3);
        assert_eq!(slots[&third.id], 4);
    }

    #[test]
    fn rejects_duplicate_components_and_overlapping_functions() {
        let head_id = Uuid::new_v4();
        let mut first = channel(head_id, ChannelResolution::U16, vec![2]);
        let second = channel(head_id, ChannelResolution::U16, vec![2]);
        first.functions.push(ChannelFunction {
            id: Uuid::new_v4(),
            name: "Conflict".into(),
            dmx_from: 100,
            dmx_to: 200,
            attribute: AttributeKey("strobe".into()),
            priority: 100,
            behavior: ChannelFunctionBehavior::Fixed {
                semantic_id: "strobe".into(),
                label: "Strobe".into(),
                raw_value: 150,
            },
        });
        assert!(
            matches!(first.validate(), Err(ProfileError::Invalid(message)) if message.contains("overlap"))
        );
        first.functions.pop();
        let mode = FixtureMode {
            id: Uuid::new_v4(),
            name: "Mode".into(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: 4,
            }],
            heads: vec![FixtureHead {
                id: head_id,
                name: "Main".into(),
                master_shared: true,
            }],
            channels: vec![first, second],
            color_systems: vec![],
            control_actions: vec![],
            geometry: GeometryGraph::default(),
        };
        assert!(
            matches!(mode.primary_slots(), Err(ProfileError::Invalid(message)) if message.contains("duplicated"))
        );
    }

    #[test]
    fn blank_profile_has_one_default_mode_and_head() {
        let draft = FixtureProfile::blank();
        assert_eq!(draft.modes.len(), 1);
        assert_eq!(draft.modes[0].name, "Default");
        assert_eq!(draft.modes[0].heads.len(), 1);
    }

    #[test]
    fn mode_rejects_more_than_one_master_shared_head() {
        let mut mode = FixtureProfile::blank().modes.remove(0);
        mode.heads.push(FixtureHead {
            id: Uuid::new_v4(),
            name: "Shared 2".into(),
            master_shared: true,
        });

        assert!(matches!(
            mode.validate(),
            Err(ProfileError::Invalid(message))
                if message == "at most one head can be master/shared"
        ));
    }

    #[test]
    fn mode_rejects_a_channel_that_references_a_missing_split() {
        let mut mode = FixtureProfile::blank().modes.remove(0);
        mode.channels.push(channel(mode.heads[0].id, ChannelResolution::U8, vec![]));
        mode.channels[0].split = 2;

        assert!(matches!(
            mode.validate(),
            Err(ProfileError::Invalid(message))
                if message == "head references a missing split"
        ));
    }

    #[test]
    fn legacy_migration_derives_invert_aware_full_white_and_open_wheel_highlight() {
        let attributes = [
            ("intensity", 0.0, false),
            ("color.red", 0.0, false),
            ("color.green", 0.0, false),
            ("color.blue", 0.0, false),
            ("color.white", 0.0, false),
            ("color.cyan", 0.0, true),
            ("color.magenta", 0.0, false),
            ("color.yellow", 0.0, false),
            ("color.emitter.red", 0.0, false),
            ("color.emitter.green", 0.0, false),
            ("color.emitter.blue", 0.0, false),
            ("color.wheel.1", 7.0 / 255.0, false),
            ("pan", 0.5, false),
        ];
        let parameters = attributes
            .iter()
            .enumerate()
            .map(|(offset, (attribute, default, invert))| Parameter {
                attribute: AttributeKey((*attribute).into()),
                components: vec![ChannelComponent {
                    offset: offset as u16,
                    byte_order: ByteOrder::MsbFirst,
                }],
                default: *default,
                virtual_dimmer: false,
                metadata: ParameterMetadata {
                    invert: *invert,
                    ..Default::default()
                },
                capabilities: if *attribute == "color.wheel.1" {
                    vec![Capability {
                        name: "Open / White".into(),
                        dmx_from: 12,
                        dmx_to: 18,
                        preset_family: Some("color".into()),
                    }]
                } else {
                    Vec::new()
                },
            })
            .collect::<Vec<_>>();
        let definition = FixtureDefinition {
            schema_version: 1,
            id: FixtureId::new(),
            revision: 1,
            manufacturer: "Test".into(),
            device_type: "wash".into(),
            name: "Semantic Highlight".into(),
            model: "Semantic Highlight".into(),
            mode: "Default".into(),
            footprint: parameters.len() as u16,
            heads: vec![LogicalHead {
                index: 0,
                name: "Main".into(),
                shared: true,
                parameters,
            }],
            color_calibration: Some(ColorCalibration {
                emitters: ["red", "green", "blue"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| super::super::EmitterCalibration {
                        name: name.into(),
                        xyz: match index {
                            0 => Xyz {
                                x: 1.0,
                                y: 0.0,
                                z: 0.0,
                            },
                            1 => Xyz {
                                x: 0.0,
                                y: 1.0,
                                z: 0.0,
                            },
                            _ => Xyz {
                                x: 0.0,
                                y: 0.0,
                                z: 1.0,
                            },
                        },
                        limit: 1.0,
                    })
                    .collect(),
                correction_matrix: identity_color_correction(),
            }),
            physical: FixturePhysicalProperties::default(),
            model_asset: None,
            icon_asset: None,
            hazardous: false,
            direct_control_protocols: Vec::new(),
            signal_loss_policy: SignalLossPolicy::HoldLast,
            safe_values: BTreeMap::new(),
            profile_id: None,
            mode_id: None,
            profile_snapshot: None,
        };

        let profile = FixtureProfile::from_legacy_modes(&[definition]).unwrap();
        let mode = &profile.modes[0];
        let highlights = mode
            .channels
            .iter()
            .map(|channel| (channel.attribute.0.as_str(), channel.highlight_raw))
            .collect::<HashMap<_, _>>();
        assert_eq!(highlights["intensity"], 255);
        assert_eq!(highlights["color.red"], 255);
        assert_eq!(highlights["color.green"], 255);
        assert_eq!(highlights["color.blue"], 255);
        assert_eq!(highlights["color.white"], 255);
        assert_eq!(highlights["color.cyan"], 255, "inverted no-filter endpoint");
        assert_eq!(highlights["color.magenta"], 0);
        assert_eq!(highlights["color.yellow"], 0);
        assert_eq!(highlights["color.wheel.1"], 15);
        assert_eq!(highlights["pan"], 128);
        let calibrated_white = mode
            .resolve_color(mode.heads[0].id, SEMANTIC_WHITE_XYZ)
            .unwrap();
        for attribute in [
            "color.emitter.red",
            "color.emitter.green",
            "color.emitter.blue",
        ] {
            let channel = mode
                .channels
                .iter()
                .find(|channel| channel.attribute.0 == attribute)
                .unwrap();
            assert_eq!(channel.highlight_raw, calibrated_white[&channel.id]);
        }
    }

    #[test]
    fn authored_schema_v2_highlight_raw_is_not_rederived() {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Authored Highlight".into();
        profile.revision = 4;
        let mode = &mut profile.modes[0];
        let mut authored = channel(mode.heads[0].id, ChannelResolution::U8, Vec::new());
        authored.attribute = AttributeKey("color.cyan".into());
        authored.highlight_raw = 73;
        mode.channels = vec![authored.clone()];

        let encoded = serde_json::to_string(&profile).unwrap();
        let decoded: FixtureProfile = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.modes[0].channels[0].highlight_raw, 73);
        let definition = decoded.resolved_definition(decoded.modes[0].id).unwrap();
        assert_eq!(
            definition.profile_snapshot.unwrap().modes[0].channels[0].highlight_raw,
            73
        );
    }

    #[test]
    fn legacy_head_split_migrates_to_channels_and_serializes_canonically() {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Legacy split".into();
        let mode = &mut profile.modes[0];
        mode.channels = vec![channel(mode.heads[0].id, ChannelResolution::U8, vec![])];
        let mut value = serde_json::to_value(&profile).unwrap();
        let mode = &mut value["modes"][0];
        mode["heads"][0]["split"] = serde_json::json!(1);
        mode["channels"][0].as_object_mut().unwrap().remove("split");

        let migrated: FixtureProfile = serde_json::from_value(value).unwrap();
        assert_eq!(migrated.modes[0].channels[0].split, 1);
        let canonical = serde_json::to_value(migrated).unwrap();
        assert!(canonical["modes"][0]["heads"][0].get("split").is_none());
        assert_eq!(canonical["modes"][0]["channels"][0]["split"], 1);
    }

    #[test]
    fn exact_raw_values_encode_msb_first_at_every_supported_resolution() {
        let cases = [
            (ChannelResolution::U8, 0x0000_00ab, vec![], vec![0xab]),
            (
                ChannelResolution::U16,
                0x0000_abcd,
                vec![2],
                vec![0xab, 0xcd],
            ),
            (
                ChannelResolution::U24,
                0x00ab_cdef,
                vec![2, 3],
                vec![0xab, 0xcd, 0xef],
            ),
            (
                ChannelResolution::U32,
                0xabcd_ef12,
                vec![2, 3, 4],
                vec![0xab, 0xcd, 0xef, 0x12],
            ),
        ];
        for (resolution, expected_raw, secondary_slots, expected_bytes) in cases {
            let head_id = Uuid::new_v4();
            let fixture_channel = channel(head_id, resolution, secondary_slots);
            let mode = FixtureMode {
                id: Uuid::new_v4(),
                name: "Mode".into(),
                notes: String::new(),
                splits: vec![FixtureSplit {
                    number: 1,
                    footprint: resolution.bytes() as u16,
                }],
                heads: vec![FixtureHead {
                    id: head_id,
                    name: "Main".into(),
                    master_shared: true,
                }],
                channels: vec![fixture_channel.clone()],
                color_systems: vec![],
                control_actions: vec![],
                geometry: GeometryGraph::default(),
            };
            let values = HashMap::from([(
                AttributeKey::intensity(),
                AttributeValue::RawDmxExact(expected_raw),
            )]);
            let raw = mode.resolve_channel_raw(
                &fixture_channel,
                &values,
                false,
                None,
                ChannelScales::default(),
            );
            assert_eq!(raw, expected_raw);
            let mut frame = [0_u8; 512];
            mode.encode_channel(&mut frame, 5, &fixture_channel, raw)
                .unwrap();
            assert_eq!(
                &frame[4..4 + expected_bytes.len()],
                expected_bytes.as_slice()
            );
        }
    }

    #[test]
    fn multi_function_priority_release_static_and_highlight_are_deterministic() {
        let head_id = Uuid::new_v4();
        let mut fixture_channel = channel(head_id, ChannelResolution::U8, vec![]);
        fixture_channel.highlight_raw = 240;
        fixture_channel.reacts_to_group_master = true;
        fixture_channel.reacts_to_grand_master = true;
        fixture_channel.functions = vec![
            ChannelFunction {
                id: Uuid::new_v4(),
                name: "Dimmer".into(),
                dmx_from: 0,
                dmx_to: 127,
                attribute: AttributeKey::intensity(),
                priority: 0,
                behavior: ChannelFunctionBehavior::Continuous {
                    physical_min: 0.0,
                    physical_max: 1.0,
                    unit: None,
                },
            },
            ChannelFunction {
                id: Uuid::new_v4(),
                name: "Open".into(),
                dmx_from: 128,
                dmx_to: 255,
                attribute: AttributeKey("shutter".into()),
                priority: 10,
                behavior: ChannelFunctionBehavior::Fixed {
                    semantic_id: "open".into(),
                    label: "Open".into(),
                    raw_value: 200,
                },
            },
        ];
        let mode = FixtureMode {
            id: Uuid::new_v4(),
            name: "Mode".into(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: 1,
            }],
            heads: vec![FixtureHead {
                id: head_id,
                name: "Main".into(),
                master_shared: true,
            }],
            channels: vec![fixture_channel.clone()],
            color_systems: vec![],
            control_actions: vec![],
            geometry: GeometryGraph::default(),
        };
        mode.validate().unwrap();
        let mut values = HashMap::from([
            (AttributeKey::intensity(), AttributeValue::Normalized(0.5)),
            (
                AttributeKey("shutter".into()),
                AttributeValue::Discrete("open".into()),
            ),
        ]);
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &values,
                false,
                None,
                ChannelScales::default(),
            ),
            200
        );
        values.remove(&AttributeKey("shutter".into()));
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &values,
                false,
                None,
                ChannelScales::default(),
            ),
            64
        );
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &values,
                true,
                Some(220),
                ChannelScales {
                    virtual_intensity: 0.0,
                    sequence_master: 0.0,
                    group_master: 0.5,
                    grand_master: 0.5,
                },
            ),
            110,
            "Highlight bypasses virtual intensity, sequence masters, and Group Masters; Grand Master remains above it"
        );

        fixture_channel.behavior = ChannelBehavior::Static;
        fixture_channel.default_raw = 37;
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &values,
                false,
                None,
                ChannelScales::default(),
            ),
            37
        );
    }

    #[test]
    fn invert_scales_semantic_ranges_before_inversion_and_preserves_exact_raw_values() {
        let head_id = Uuid::new_v4();
        let mut fixture_channel = channel(head_id, ChannelResolution::U8, vec![]);
        fixture_channel.invert = true;
        fixture_channel.default_raw = 37;
        fixture_channel.highlight_raw = 211;
        fixture_channel.functions = vec![
            ChannelFunction {
                id: Uuid::new_v4(),
                name: "Dimmer range".into(),
                dmx_from: 10,
                dmx_to: 109,
                attribute: AttributeKey::intensity(),
                priority: 0,
                behavior: ChannelFunctionBehavior::Continuous {
                    physical_min: 0.0,
                    physical_max: 1.0,
                    unit: Some("percent".into()),
                },
            },
            ChannelFunction {
                id: Uuid::new_v4(),
                name: "Open".into(),
                dmx_from: 110,
                dmx_to: 179,
                attribute: AttributeKey("shutter".into()),
                priority: 100,
                behavior: ChannelFunctionBehavior::Fixed {
                    semantic_id: "open".into(),
                    label: "Open".into(),
                    raw_value: 150,
                },
            },
            ChannelFunction {
                id: Uuid::new_v4(),
                name: "Pattern".into(),
                dmx_from: 180,
                dmx_to: 255,
                attribute: AttributeKey("gobo".into()),
                priority: 100,
                behavior: ChannelFunctionBehavior::Indexed {
                    semantic_id: "dots".into(),
                    label: "Dots".into(),
                    raw_value: 200,
                },
            },
        ];
        let mode = FixtureMode {
            id: Uuid::new_v4(),
            name: "Mode".into(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: 1,
            }],
            heads: vec![FixtureHead {
                id: head_id,
                name: "Main".into(),
                master_shared: true,
            }],
            channels: vec![fixture_channel.clone()],
            color_systems: vec![],
            control_actions: vec![],
            geometry: GeometryGraph::default(),
        };
        mode.validate().unwrap();

        let semantic =
            HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(0.5))]);
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &semantic,
                false,
                None,
                ChannelScales {
                    grand_master: 0.5,
                    ..Default::default()
                },
            ),
            84,
            "the semantic value is scaled from 10 toward 109 before inversion inside that range"
        );
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &semantic,
                false,
                None,
                ChannelScales {
                    grand_master: 0.0,
                    ..Default::default()
                },
            ),
            109
        );

        for (values, expected) in [
            (
                HashMap::from([(AttributeKey::intensity(), AttributeValue::RawDmxExact(17))]),
                17,
            ),
            (
                HashMap::from([(
                    AttributeKey("shutter".into()),
                    AttributeValue::Discrete("open".into()),
                )]),
                150,
            ),
            (
                HashMap::from([(
                    AttributeKey("gobo".into()),
                    AttributeValue::Discrete("dots".into()),
                )]),
                200,
            ),
            (
                HashMap::from([(
                    FixtureMode::control_action_attribute(fixture_channel.id),
                    AttributeValue::RawDmxExact(23),
                )]),
                23,
            ),
        ] {
            assert_eq!(
                mode.resolve_channel_raw(
                    &fixture_channel,
                    &values,
                    false,
                    None,
                    ChannelScales::default(),
                ),
                expected
            );
        }
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &HashMap::new(),
                true,
                Some(211),
                ChannelScales::default(),
            ),
            211
        );
        let mut static_channel = fixture_channel.clone();
        static_channel.behavior = ChannelBehavior::Static;
        assert_eq!(
            mode.resolve_channel_raw(
                &static_channel,
                &HashMap::new(),
                false,
                None,
                ChannelScales::default(),
            ),
            37
        );
        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &HashMap::from([(AttributeKey::intensity(), AttributeValue::RawDmxExact(17),)]),
                false,
                None,
                ChannelScales {
                    grand_master: 0.5,
                    ..Default::default()
                },
            ),
            136,
            "an exact raw value moves toward inverted physical off instead of being reinterpreted"
        );
    }

    #[test]
    fn typed_control_action_owns_its_exact_channel_without_losing_function_precision() {
        let head_id = Uuid::new_v4();
        let mut fixture_channel = channel(head_id, ChannelResolution::U16, vec![2]);
        fixture_channel.functions = vec![ChannelFunction {
            id: Uuid::new_v4(),
            name: "High priority fixed value".into(),
            dmx_from: 0,
            dmx_to: 65_535,
            attribute: AttributeKey("shutter".into()),
            priority: 250,
            behavior: ChannelFunctionBehavior::Fixed {
                semantic_id: "open".into(),
                label: "Open".into(),
                raw_value: 40_000,
            },
        }];
        let mode = FixtureMode {
            id: Uuid::new_v4(),
            name: "Mode".into(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: 2,
            }],
            heads: vec![FixtureHead {
                id: head_id,
                name: "Main".into(),
                master_shared: true,
            }],
            channels: vec![fixture_channel.clone()],
            color_systems: vec![],
            control_actions: vec![],
            geometry: GeometryGraph::default(),
        };
        let action_attribute = FixtureMode::control_action_attribute(fixture_channel.id);
        let values = HashMap::from([
            (
                AttributeKey("shutter".into()),
                AttributeValue::Discrete("open".into()),
            ),
            (
                action_attribute.clone(),
                AttributeValue::RawDmxExact(0x1234),
            ),
        ]);

        assert_eq!(
            mode.resolve_channel_raw(
                &fixture_channel,
                &values,
                false,
                None,
                ChannelScales::default(),
            ),
            0x1234
        );
        assert_eq!(
            mode.active_attribute_for_channel(&fixture_channel, &values),
            Some(&action_attribute)
        );
    }

    #[test]
    fn geometry_motion_uses_physical_range_without_changing_profile_data() {
        let node_id = Uuid::new_v4();
        let graph = GeometryGraph {
            nodes: vec![GeometryNode {
                id: node_id,
                name: "Yoke".into(),
                parent_id: None,
                transform: Transform3::default(),
                pivot: Vector3::default(),
                glb_node: None,
                motion: Some(GeometryMotion {
                    attribute: AttributeKey("pan".into()),
                    kind: GeometryMotionKind::Rotation,
                    axis: Vector3 {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    },
                    physical_min: -270.0,
                    physical_max: 270.0,
                }),
            }],
            emitters: vec![],
        };
        let values =
            HashMap::from([(AttributeKey("pan".into()), AttributeValue::Normalized(0.75))]);
        assert_eq!(
            graph.resolved_transforms(&values)[&node_id]
                .rotation_degrees
                .y,
            135.0
        );
        assert_eq!(graph.nodes[0].transform.rotation_degrees.y, 0.0);
    }

    #[test]
    fn additive_color_applies_response_drive_limit_inversion_and_gamut_clipping() {
        let mut profile = FixtureProfile::blank();
        let mode = &mut profile.modes[0];
        let head_id = mode.heads[0].id;
        mode.splits[0].footprint = 3;
        mode.channels = ["red", "green", "blue"]
            .into_iter()
            .map(|name| {
                let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
                channel.attribute = AttributeKey(format!("color.{name}"));
                channel
            })
            .collect();
        mode.channels[0].invert = true;
        mode.color_systems = vec![HeadColorSystem {
            head_id,
            correction_matrix: identity_color_correction(),
            system: ColorSystem::Additive {
                emitters: mode
                    .channels
                    .iter()
                    .enumerate()
                    .map(|(index, channel)| EmitterBinding {
                        channel_id: channel.id,
                        name: channel.attribute.0.clone(),
                        xyz: match index {
                            0 => Xyz {
                                x: 1.0,
                                y: 0.0,
                                z: 0.0,
                            },
                            1 => Xyz {
                                x: 0.0,
                                y: 1.0,
                                z: 0.0,
                            },
                            _ => Xyz {
                                x: 0.0,
                                y: 0.0,
                                z: 1.0,
                            },
                        },
                        maximum_level: if index == 0 { 0.5 } else { 1.0 },
                        response_curve: if index == 0 { 2.0 } else { 1.0 },
                        visible: true,
                    })
                    .collect(),
            },
        }];
        mode.validate().unwrap();

        let resolved = mode
            .resolve_color(
                head_id,
                Xyz {
                    x: 0.25,
                    y: 0.0,
                    z: 0.0,
                },
            )
            .unwrap();
        assert_eq!(resolved[&mode.channels[0].id], 127);
        assert_eq!(resolved[&mode.channels[1].id], 0);
        assert_eq!(resolved[&mode.channels[2].id], 0);

        let clipped = mode
            .resolve_color(
                head_id,
                Xyz {
                    x: 2.0,
                    y: 0.0,
                    z: 0.5,
                },
            )
            .unwrap();
        assert_eq!(clipped[&mode.channels[0].id], 127);
        assert_eq!(clipped[&mode.channels[1].id], 0);
        assert_eq!(clipped[&mode.channels[2].id], 128);
    }

    #[test]
    fn subtractive_color_uses_cmy_fallback_and_honors_continuous_inversion() {
        let mut profile = FixtureProfile::blank();
        let mode = &mut profile.modes[0];
        let head_id = mode.heads[0].id;
        mode.splits[0].footprint = 3;
        mode.channels = ["cyan", "magenta", "yellow"]
            .into_iter()
            .map(|name| {
                let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
                channel.attribute = AttributeKey(format!("color.{name}"));
                channel
            })
            .collect();
        mode.channels[1].invert = true;
        mode.color_systems = vec![HeadColorSystem {
            head_id,
            correction_matrix: identity_color_correction(),
            system: ColorSystem::Subtractive {
                cyan_channel_id: mode.channels[0].id,
                magenta_channel_id: mode.channels[1].id,
                yellow_channel_id: mode.channels[2].id,
            },
        }];
        mode.validate().unwrap();

        let resolved = mode
            .resolve_color(head_id, crate::srgb_to_xyz(1.0, 0.0, 0.0))
            .unwrap();
        assert_eq!(resolved[&mode.channels[0].id], 0);
        assert_eq!(resolved[&mode.channels[1].id], 0);
        assert_eq!(resolved[&mode.channels[2].id], 255);
    }

    #[test]
    fn discrete_color_wheel_selects_measured_slot_as_an_exact_fixture_raw_value() {
        let mut profile = FixtureProfile::blank();
        let mode = &mut profile.modes[0];
        let head_id = mode.heads[0].id;
        let mut wheel = channel(head_id, ChannelResolution::U8, vec![]);
        wheel.attribute = AttributeKey("color.wheel.1".into());
        wheel.invert = true;
        let wheel_id = wheel.id;
        mode.channels = vec![wheel];
        let red = crate::srgb_to_xyz(1.0, 0.0, 0.0);
        let blue = crate::srgb_to_xyz(0.0, 0.0, 1.0);
        mode.color_systems = vec![HeadColorSystem {
            head_id,
            correction_matrix: identity_color_correction(),
            system: ColorSystem::DiscreteWheel {
                channel_id: wheel_id,
                slots: vec![
                    ColorWheelSlot {
                        semantic_id: "red".into(),
                        label: "Red".into(),
                        dmx_from: 10,
                        dmx_to: 40,
                        measured_xyz: Some(red),
                    },
                    ColorWheelSlot {
                        semantic_id: "blue".into(),
                        label: "Blue".into(),
                        dmx_from: 100,
                        dmx_to: 140,
                        measured_xyz: Some(blue),
                    },
                ],
            },
        }];
        mode.validate().unwrap();

        assert_eq!(mode.resolve_color(head_id, blue).unwrap()[&wheel_id], 120);
    }

    #[test]
    fn rejects_non_finite_and_negative_additive_calibration() {
        let valid = additive_color_mode();
        valid.validate().unwrap();

        for invalid in [f32::NAN, f32::INFINITY, -0.1] {
            let mut mode = valid.clone();
            additive_emitter(&mut mode).xyz.x = invalid;
            assert!(matches!(
                mode.validate(),
                Err(ProfileError::Invalid(message))
                    if message.contains("additive emitter calibration")
            ));

            let mut mode = valid.clone();
            additive_emitter(&mut mode).maximum_level = invalid;
            assert!(matches!(
                mode.validate(),
                Err(ProfileError::Invalid(message))
                    if message.contains("additive emitter calibration")
            ));

            let mut mode = valid.clone();
            additive_emitter(&mut mode).response_curve = invalid;
            assert!(matches!(
                mode.validate(),
                Err(ProfileError::Invalid(message))
                    if message.contains("additive emitter calibration")
            ));
        }
    }

    #[test]
    fn rejects_invalid_discrete_wheel_slot_metadata_and_ranges() {
        let valid = discrete_color_mode();
        valid.validate().unwrap();

        let mut empty = valid.clone();
        wheel_slots(&mut empty).clear();
        assert!(empty.validate().is_err());

        let mut empty_semantic_id = valid.clone();
        wheel_slots(&mut empty_semantic_id)[0].semantic_id = "  ".into();
        assert!(empty_semantic_id.validate().is_err());

        let mut duplicate_semantic_id = valid.clone();
        wheel_slots(&mut duplicate_semantic_id)[1].semantic_id = "red".into();
        assert!(duplicate_semantic_id.validate().is_err());

        let mut empty_label = valid.clone();
        wheel_slots(&mut empty_label)[0].label = "  ".into();
        assert!(empty_label.validate().is_err());

        let mut reversed_range = valid.clone();
        wheel_slots(&mut reversed_range)[0].dmx_from = 41;
        assert!(reversed_range.validate().is_err());

        let mut unsorted = valid.clone();
        wheel_slots(&mut unsorted).swap(0, 1);
        assert!(matches!(
            unsorted.validate(),
            Err(ProfileError::Invalid(message)) if message.contains("sorted")
        ));

        let mut overlapping = valid.clone();
        wheel_slots(&mut overlapping)[1].dmx_from = 40;
        assert!(matches!(
            overlapping.validate(),
            Err(ProfileError::Invalid(message)) if message.contains("non-overlapping")
        ));

        let mut out_of_range = valid.clone();
        wheel_slots(&mut out_of_range)[1].dmx_to = 256;
        assert!(out_of_range.validate().is_err());

        for invalid in [f32::NAN, f32::INFINITY, -0.1] {
            let mut invalid_measurement = valid.clone();
            wheel_slots(&mut invalid_measurement)[0]
                .measured_xyz
                .as_mut()
                .unwrap()
                .y = invalid;
            assert!(invalid_measurement.validate().is_err());
        }
    }

    #[test]
    fn visual_only_profiles_require_zero_footprint_and_no_dmx_behavior() {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Venue".into();
        profile.name = "Stage Element".into();
        profile.patch_policy = PatchPolicy::VisualOnly;
        profile.modes[0].splits[0].footprint = 0;
        profile.validate().unwrap();
        assert_eq!(
            profile
                .resolved_definition(profile.modes[0].id)
                .unwrap()
                .footprint,
            0
        );

        profile.modes[0].splits[0].footprint = 1;
        assert!(profile.validate().is_err());
        profile.modes[0].splits[0].footprint = 0;
        let head = profile.modes[0].heads[0].id;
        profile.modes[0].channels.push(FixtureChannel {
            id: Uuid::new_v4(),
            head_id: head,
            split: 1,
            attribute: AttributeKey::intensity(),
            resolution: ChannelResolution::U8,
            secondary_slots: vec![],
            default_raw: 0,
            highlight_raw: 255,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: true,
            reacts_to_group_master: true,
            reacts_to_grand_master: true,
            behavior: ChannelBehavior::Controlled,
            functions: vec![],
        });
        assert!(profile.validate().is_err());
    }

    #[test]
    fn missing_patch_and_model_policy_fields_decode_to_legacy_defaults() {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Generic".into();
        profile.name = "Dimmer".into();
        let mut value = serde_json::to_value(profile).unwrap();
        value.as_object_mut().unwrap().remove("patch_policy");
        value.as_object_mut().unwrap().remove("model_units");
        let decoded: FixtureProfile = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.patch_policy, PatchPolicy::Dmx);
        assert_eq!(decoded.model_units, ModelUnits::Auto);
    }
}
