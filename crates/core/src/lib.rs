#![forbid(unsafe_code)]
//! Shared, transport-neutral lighting domain primitives.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::{fmt::Debug, sync::Arc, time::Instant};
use thiserror::Error;
use uuid::Uuid;

pub type Revision = u64;
pub type Universe = u16;
pub type DmxAddress = u16;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}
id!(UserId);
id!(SessionId);
id!(ShowId);
id!(FixtureId);
id!(CueListId);
id!(ProgrammerId);

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

/// Canonical metadata shared by fixture profiles and programmer surfaces.  The stable `id` is
/// what is persisted; labels and default units may evolve without rewriting show data.
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
    AttributeDescriptor {
        id: "intensity",
        label: "Intensity",
        family: AttributeClass::Intensity,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color",
        label: "Color",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Color,
        default_unit: None,
    },
    AttributeDescriptor {
        id: "color.red",
        label: "Red",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.green",
        label: "Green",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.blue",
        label: "Blue",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.cyan",
        label: "Cyan",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.magenta",
        label: "Magenta",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.yellow",
        label: "Yellow",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.amber",
        label: "Amber",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.white",
        label: "White",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.uv",
        label: "UV",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "color.wheel.1",
        label: "Color Wheel 1",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Indexed,
        default_unit: None,
    },
    AttributeDescriptor {
        id: "color.wheel.2",
        label: "Color Wheel 2",
        family: AttributeClass::Color,
        value_type: AttributeValueType::Indexed,
        default_unit: None,
    },
    AttributeDescriptor {
        id: "pan",
        label: "Pan",
        family: AttributeClass::Position,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("deg"),
    },
    AttributeDescriptor {
        id: "tilt",
        label: "Tilt",
        family: AttributeClass::Position,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("deg"),
    },
    AttributeDescriptor {
        id: "beam",
        label: "Beam",
        family: AttributeClass::Beam,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "focus",
        label: "Focus",
        family: AttributeClass::Focus,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "zoom",
        label: "Zoom",
        family: AttributeClass::Beam,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("deg"),
    },
    AttributeDescriptor {
        id: "iris",
        label: "Iris",
        family: AttributeClass::Beam,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("percent"),
    },
    AttributeDescriptor {
        id: "gobo.1",
        label: "Gobo 1",
        family: AttributeClass::Beam,
        value_type: AttributeValueType::Indexed,
        default_unit: None,
    },
    AttributeDescriptor {
        id: "gobo.2",
        label: "Gobo 2",
        family: AttributeClass::Beam,
        value_type: AttributeValueType::Indexed,
        default_unit: None,
    },
    AttributeDescriptor {
        id: "shutter",
        label: "Shutter",
        family: AttributeClass::Beam,
        value_type: AttributeValueType::Indexed,
        default_unit: None,
    },
    AttributeDescriptor {
        id: "strobe",
        label: "Strobe",
        family: AttributeClass::Beam,
        value_type: AttributeValueType::Continuous,
        default_unit: Some("hz"),
    },
    AttributeDescriptor {
        id: "control",
        label: "Control",
        family: AttributeClass::Control,
        value_type: AttributeValueType::Control,
        default_unit: None,
    },
];

pub fn attribute_descriptor(key: &AttributeKey) -> AttributeDescriptor {
    ATTRIBUTE_REGISTRY
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == key.0)
        .unwrap_or(AttributeDescriptor {
            id: "custom",
            label: "Custom",
            family: AttributeClass::Custom,
            value_type: AttributeValueType::Continuous,
            default_unit: None,
        })
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Debug, Error)]
pub enum LightError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("revision conflict: expected {expected}, current {current}")]
    RevisionConflict {
        expected: Revision,
        current: Revision,
    },
    #[error("not found: {0}")]
    NotFound(String),
}

/// Monotonic clock used by render loops. Wall clock is intentionally not used for frame deadlines.
#[derive(Clone, Debug)]
pub struct EngineClock {
    started: Instant,
}

/// Application time used for lighting behavior. Scheduler deadlines deliberately use
/// `Instant` instead so a manually controlled test clock cannot affect real-time I/O health.
pub trait ApplicationClock: Debug + Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub type SharedClock = Arc<dyn ApplicationClock>;

#[derive(Debug, Default)]
pub struct SystemClock;

impl ApplicationClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug)]
pub struct ManualClock {
    now: RwLock<DateTime<Utc>>,
}

impl ManualClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: RwLock::new(now),
        }
    }

    pub fn set(&self, now: DateTime<Utc>) {
        *self.now.write().expect("manual clock lock poisoned") = now;
    }

    pub fn advance_millis(&self, millis: i64) -> DateTime<Utc> {
        let mut now = self.now.write().expect("manual clock lock poisoned");
        *now += chrono::Duration::milliseconds(millis);
        *now
    }
}

impl ApplicationClock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.read().expect("manual clock lock poisoned")
    }
}
impl Default for EngineClock {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}
impl EngineClock {
    pub fn elapsed_micros(&self) -> u64 {
        self.started.elapsed().as_micros() as u64
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn manual_application_time_only_moves_when_advanced() {
        let start = DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = ManualClock::new(start);
        assert_eq!(clock.now(), start);
        assert_eq!(clock.now(), start);
        assert_eq!(
            clock.advance_millis(30_000),
            start + chrono::Duration::seconds(30)
        );
    }
}
