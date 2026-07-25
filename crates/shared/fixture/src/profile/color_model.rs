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
