use super::{FixtureHead, FixtureMode, FixtureSplit, GeometryGraph, HeadColorSystem};
use light_core::AttributeKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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
        let canonical: FixtureModeCanonical =
            serde_json::from_value(value).map_err(<D::Error as serde::de::Error>::custom)?;
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

/// Portable operator meaning for fixture-control actions. `Custom` preserves profiles authored
/// before semantic control actions were introduced and actions which intentionally only appear in
/// Direct Mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlActionSemantic {
    #[default]
    Custom,
    LampOn,
    LampOff,
    Reset,
    FanAuto,
    FanLow,
    FanHigh,
    FanMax,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlAction {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub semantic: ControlActionSemantic,
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
