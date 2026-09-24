use super::ChannelResolution;
use crate::Capability;
use light_core::{AttributeKey, Xyz};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeadColorSystem {
    pub head_id: Uuid,
    #[serde(default = "identity_color_correction")]
    pub correction_matrix: [[f32; 3]; 3],
    pub system: ColorSystem,
    /// How far Color Intent may trust this system's colour data. Profiles written before the
    /// declaration existed read as [`ColorCalibrationStatus::Nominal`] at revision zero.
    #[serde(default)]
    pub calibration: ColorSystemCalibration,
}

/// Where a colour system's data comes from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorCalibrationStatus {
    /// Measured on the fixture with a colorimeter or spectrometer.
    Measured,
    /// Taken from a datasheet or typical values: the colour is right, the exact shade may not be.
    #[default]
    Nominal,
    /// Not enough data to promise a colour; Color Intent resolves from channel names only.
    Uncalibrated,
}

/// The versioned calibration declaration a colour system carries in its fixture package.
///
/// A show stores its Color Intent as device-independent colour and each patched fixture keeps
/// the profile snapshot it was patched with, so a new calibration revision changes a fixture's
/// output only when the operator updates that fixture's profile; the stored intent never changes.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ColorSystemCalibration {
    #[serde(default)]
    pub status: ColorCalibrationStatus,
    /// Incremented whenever the colour data of this system changes.
    #[serde(default)]
    pub revision: u32,
    /// Free text naming the instrument, datasheet, or person behind the data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Measured output of a CMY engine: the open beam, and the beam with each flag fully in on its
/// own. Colour Intent models the flags as independent filters between those measurements.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubtractiveCalibration {
    pub open_xyz: Xyz,
    pub cyan_xyz: Xyz,
    pub magenta_xyz: Xyz,
    pub yellow_xyz: Xyz,
}

pub(super) fn identity_color_correction() -> [[f32; 3]; 3] {
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
        /// Measured filter output. Without it the flags are treated as ideal sRGB complements.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filters: Option<SubtractiveCalibration>,
    },
    /// A fixture-native hue/saturation coordinate system. Brightness is optional because many
    /// fixtures expose H/S alongside an independent intensity channel while true HSI fixtures
    /// carry the third coordinate in the color system itself.
    HueSaturation {
        hue_channel_id: Uuid,
        saturation_channel_id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intensity_channel_id: Option<Uuid>,
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

pub(super) fn valid_measured_xyz(xyz: Xyz) -> bool {
    [xyz.x, xyz.y, xyz.z]
        .into_iter()
        .all(|component| component.is_finite() && component >= 0.0)
}

pub(super) fn legacy_emitter_is_visible(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    !matches!(name.as_str(), "uv" | "ir")
        && !name.contains("ultraviolet")
        && !name.contains("infrared")
}

pub(super) const SEMANTIC_WHITE_XYZ: Xyz = Xyz {
    x: 0.950_47,
    y: 1.0,
    z: 1.088_83,
};

fn semantic_endpoint(max: u32, full: bool, invert: bool) -> u32 {
    if full != invert { max } else { 0 }
}

pub(super) fn identifies_open_or_white(value: &str) -> bool {
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

pub(super) fn semantic_highlight_raw(
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
    match &*attribute.0 {
        "color.red" | "color.green" | "color.blue" | "color.white" | "color.cold_white"
        | "color.warm_white" | "color.amber" => {
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
    /// Whether Color Intent may park the wheel here to show a colour. Unset, a slot counts as
    /// steady unless its name describes a split, scroll, rotation, or effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steady: Option<bool>,
}
