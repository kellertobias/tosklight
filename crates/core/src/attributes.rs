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

/// Resolves ordered spread control points across a `count`-strong selection using the
/// deterministic anchor rule (docs/plans/Next/50):
///
/// 1. The first and last control points anchor the first and last items.
/// 2. Interior control point `j` has the ideal position `j × (count - 1) / (points - 1)`,
///    compared exactly as a rational so floating-point error cannot move an anchor: an integer
///    position anchors that item, an exact half-position anchors **both** adjacent items
///    (midpoint expansion), and any other position anchors the nearest item.
/// 3. Items between adjacent anchors are interpolated in equal steps.
///
/// Two-point spreads keep their established endpoint-interpolation meaning (the rule reduces to
/// it). More control points than items cannot place every point — entry paths reject that input;
/// this total resolver degrades to plain linear sampling so stored legacy spreads still render.
pub fn resolve_spread(points: &[f32], count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    let first = points.first().copied().unwrap_or(0.0);
    if points.len() <= 1 || count == 1 {
        return vec![first; count];
    }
    if points.len() > count {
        return (0..count)
            .map(|index| linear(points, index, count))
            .collect();
    }
    // Exact anchor placement: ideal position of point j is j*(count-1)/(points-1).
    let mut anchors: Vec<(usize, f32)> = Vec::with_capacity(points.len() + 2);
    let denominator = points.len() - 1;
    for (j, value) in points.iter().enumerate() {
        let numerator = j * (count - 1);
        let item = numerator / denominator;
        let remainder = numerator % denominator;
        if remainder == 0 {
            anchors.push((item, *value));
        } else if remainder * 2 == denominator {
            anchors.push((item, *value));
            anchors.push((item + 1, *value));
        } else if remainder * 2 > denominator {
            anchors.push((item + 1, *value));
        } else {
            anchors.push((item, *value));
        }
    }
    let mut resolved = vec![0.0_f32; count];
    for window in anchors.windows(2) {
        let (left_item, left_value) = window[0];
        let (right_item, right_value) = window[1];
        resolved[left_item] = left_value;
        resolved[right_item] = right_value;
        let span = right_item.saturating_sub(left_item);
        for step in 1..span {
            // Symmetric weighted form so a reversed selection resolves to the exact mirrored
            // bytes (`a + (b-a)*t` is not float-symmetric).
            resolved[left_item + step] =
                (left_value * (span - step) as f32 + right_value * step as f32) / span as f32;
        }
    }
    resolved
}

fn linear(points: &[f32], index: usize, count: usize) -> f32 {
    let position = index as f32 * (points.len() - 1) as f32 / (count - 1) as f32;
    let left = position.floor() as usize;
    let right = position.ceil() as usize;
    points[left] + (points[right] - points[left]) * (position - left as f32)
}

/// Resolves the value at `index` of an ordered `count`-strong selection from the given control
/// points via [`resolve_spread`]. Shared by every fan-out path (command line, groups, ordered
/// selections, engine rendering of stored spreads) so all surfaces distribute identically.
pub fn spread_position(points: &[f32], index: usize, count: usize) -> f32 {
    resolve_spread(points, count)
        .get(index)
        .copied()
        .unwrap_or(points.first().copied().unwrap_or(0.0))
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

#[cfg(test)]
mod spread_tests {
    use super::{resolve_spread, spread_position};

    fn percentages(points: &[f32], count: usize) -> Vec<f32> {
        resolve_spread(points, count)
            .into_iter()
            .map(|value| (value * 100.0 * 10.0).round() / 10.0)
            .collect()
    }

    #[test]
    fn normative_table_for_100_thru_0_thru_100() {
        let points = [1.0, 0.0, 1.0];
        assert_eq!(percentages(&points, 4), [100.0, 0.0, 0.0, 100.0]);
        assert_eq!(percentages(&points, 5), [100.0, 50.0, 0.0, 50.0, 100.0]);
        assert_eq!(
            percentages(&points, 6),
            [100.0, 50.0, 0.0, 0.0, 50.0, 100.0]
        );
        assert_eq!(
            percentages(&points, 10),
            [100.0, 75.0, 50.0, 25.0, 0.0, 0.0, 25.0, 50.0, 75.0, 100.0]
        );
    }

    #[test]
    fn asymmetric_and_four_point_vectors_place_every_anchor_exactly() {
        // 10 THRU 80 THRU 20 over 5: interior ideal position 2 is an exact item.
        assert_eq!(
            percentages(&[0.1, 0.8, 0.2], 5),
            [10.0, 45.0, 80.0, 50.0, 20.0]
        );
        // Four points over 7: ideals 0, 2, 4, 6 are all integer anchors.
        assert_eq!(
            percentages(&[0.0, 1.0, 0.25, 0.75], 7),
            [0.0, 50.0, 100.0, 62.5, 25.0, 50.0, 75.0]
        );
        // Non-half nearest anchor: 3 points over 7 → interior ideal 3 exact.
        assert_eq!(
            percentages(&[0.0, 1.0, 0.0], 7),
            [0.0, 33.3, 66.7, 100.0, 66.7, 33.3, 0.0]
        );
    }

    #[test]
    fn reversed_control_points_mirror_the_resolution() {
        for count in [4_usize, 5, 6, 9, 10] {
            let forward = resolve_spread(&[0.1, 0.9, 0.3], count);
            let mut mirrored = resolve_spread(&[0.3, 0.9, 0.1], count);
            mirrored.reverse();
            assert_eq!(forward, mirrored, "count {count}");
        }
    }

    #[test]
    fn boundaries_stay_total_and_established_meanings_hold() {
        assert!(resolve_spread(&[1.0, 0.0], 0).is_empty());
        assert_eq!(resolve_spread(&[0.4, 0.8], 1), [0.4]);
        assert_eq!(resolve_spread(&[0.7], 4), [0.7, 0.7, 0.7, 0.7]);
        // Established two-point interpolation is unchanged.
        assert_eq!(percentages(&[0.0, 1.0], 5), [0.0, 25.0, 50.0, 75.0, 100.0]);
        // Equal adjacent points expand as a flat plateau.
        assert_eq!(
            percentages(&[0.0, 0.0, 1.0], 5),
            [0.0, 0.0, 0.0, 50.0, 100.0]
        );
        // More points than items degrades to linear sampling for stored legacy spreads.
        assert_eq!(percentages(&[0.0, 1.0, 0.0, 1.0], 3), [0.0, 50.0, 100.0]);
        // Repeated evaluation is byte-for-byte stable and finite.
        let first = resolve_spread(&[0.2, 0.9, 0.1], 8);
        assert_eq!(first, resolve_spread(&[0.2, 0.9, 0.1], 8));
        assert!(first.iter().all(|value| value.is_finite()));
        assert_eq!(spread_position(&[0.2, 0.9, 0.1], 0, 8), first[0]);
    }
}
