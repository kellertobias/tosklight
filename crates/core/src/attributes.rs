use crate::FixtureId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeClass {
    Intensity,
    Position,
    Color,
    Beam,
    Focus,
    Control,
    Custom,
}

/// Canonical metadata shared by fixture profiles and programmer surfaces. The stable `id` is
/// persisted; labels and default units may evolve without rewriting show data.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AttributeDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub family: AttributeClass,
    pub value_type: AttributeValueType,
    pub default_unit: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeValueType {
    Continuous,
    Color,
    Indexed,
    Control,
}

/// Built-in attribute registry. Custom attributes remain valid and use their persisted identifier
/// as the operator label until a desk extension supplies richer metadata.
pub const ATTRIBUTE_REGISTRY: &[AttributeDescriptor] = &[
    descriptor(
        "intensity",
        "Intensity",
        AttributeClass::Intensity,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color",
        "Color",
        AttributeClass::Color,
        AttributeValueType::Color,
        None,
    ),
    descriptor(
        "color.red",
        "Red",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.green",
        "Green",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.blue",
        "Blue",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.cyan",
        "Cyan",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.magenta",
        "Magenta",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.yellow",
        "Yellow",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.amber",
        "Amber",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.white",
        "White",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.uv",
        "UV",
        AttributeClass::Color,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "color.wheel.1",
        "Color Wheel 1",
        AttributeClass::Color,
        AttributeValueType::Indexed,
        None,
    ),
    descriptor(
        "color.wheel.2",
        "Color Wheel 2",
        AttributeClass::Color,
        AttributeValueType::Indexed,
        None,
    ),
    descriptor(
        "pan",
        "Pan",
        AttributeClass::Position,
        AttributeValueType::Continuous,
        Some("deg"),
    ),
    descriptor(
        "tilt",
        "Tilt",
        AttributeClass::Position,
        AttributeValueType::Continuous,
        Some("deg"),
    ),
    descriptor(
        "beam",
        "Beam",
        AttributeClass::Beam,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "focus",
        "Focus",
        AttributeClass::Focus,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "zoom",
        "Zoom",
        AttributeClass::Beam,
        AttributeValueType::Continuous,
        Some("deg"),
    ),
    descriptor(
        "iris",
        "Iris",
        AttributeClass::Beam,
        AttributeValueType::Continuous,
        Some("percent"),
    ),
    descriptor(
        "gobo.1",
        "Gobo 1",
        AttributeClass::Beam,
        AttributeValueType::Indexed,
        None,
    ),
    descriptor(
        "gobo.2",
        "Gobo 2",
        AttributeClass::Beam,
        AttributeValueType::Indexed,
        None,
    ),
    descriptor(
        "shutter",
        "Shutter",
        AttributeClass::Beam,
        AttributeValueType::Indexed,
        None,
    ),
    descriptor(
        "strobe",
        "Strobe",
        AttributeClass::Beam,
        AttributeValueType::Continuous,
        Some("hz"),
    ),
    descriptor(
        "control",
        "Control",
        AttributeClass::Control,
        AttributeValueType::Control,
        None,
    ),
];

const fn descriptor(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    value_type: AttributeValueType,
    default_unit: Option<&'static str>,
) -> AttributeDescriptor {
    AttributeDescriptor {
        id,
        label,
        family,
        value_type,
        default_unit,
    }
}

pub fn attribute_descriptor(key: &AttributeKey) -> AttributeDescriptor {
    ATTRIBUTE_REGISTRY
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == key.0)
        .unwrap_or_else(custom_descriptor)
}

const fn custom_descriptor() -> AttributeDescriptor {
    descriptor(
        "custom",
        "Custom",
        AttributeClass::Custom,
        AttributeValueType::Continuous,
        None,
    )
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeKey(pub String);

impl AttributeKey {
    pub fn intensity() -> Self {
        Self("intensity".into())
    }

    pub fn is_intensity(&self) -> bool {
        self.0 == "intensity" || self.0.ends_with(".intensity")
    }

    pub fn is_position(&self) -> bool {
        self.0 == "pan"
            || self.0 == "tilt"
            || self.0.starts_with("position.")
            || self.0.ends_with(".pan")
            || self.0.ends_with(".tilt")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Xyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AttributeValue {
    Normalized(f32),
    /// Normalized control points distributed over an ordered Group membership.
    Spread(Vec<f32>),
    Discrete(String),
    ColorXyz(Xyz),
    RawDmx(u8),
    /// Resolution-independent raw channel value used by schema-v2 fixture profiles. The fixture
    /// channel clamps this to its configured 8/16/24/32-bit range at render time.
    RawDmxExact(u32),
}

impl AttributeValue {
    pub fn normalized(&self) -> Option<f32> {
        match self {
            Self::Normalized(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Htp,
    Ltp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedValue {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    pub priority: i16,
    pub changed_at: DateTime<Utc>,
    /// Stable programmer-local edit order for values that intentionally share one timestamp.
    #[serde(default)]
    pub programmer_order: u64,
    pub merge_mode: MergeMode,
    /// Whether this direct-entry value should use the configured programmer fade.
    #[serde(default)]
    pub fade: bool,
    /// A command-specific fade override. `None` keeps the configured programmer fade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    /// A command-specific delay before the value starts fading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}
