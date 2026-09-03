use crate::FixtureId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeClass {
    Intensity,
    Position,
    Color,
    Beam,
    Shapers,
    Focus,
    Control,
    Media,
    Custom,
}

/// Canonical metadata shared by fixture profiles and programmer surfaces. The stable `id` is
/// persisted; labels and default units may evolve without rewriting show data.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AttributeDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub family: AttributeClass,
    pub value_type: AttributeValueType,
    /// Retained compatibility field for existing clients. New code should prefer `display_unit`.
    pub default_unit: Option<&'static str>,
    pub display_unit: Option<&'static str>,
    pub physical_unit: Option<&'static str>,
    /// Canonical normalized bounds before a fixture channel maps the value into its authored
    /// physical domain. `None` means the attribute is not a scalar Dynamics lane.
    pub normalized_bounds: Option<AttributeBounds>,
    /// Optional canonical physical/display domain. Fixture-authored physical ranges remain
    /// authoritative when this is `None`.
    pub domain_bounds: Option<AttributeBounds>,
    /// Cyclic scalar attributes wrap at their canonical bounds rather than clamping.
    pub cyclic: bool,
    /// Transient control actions are deliberately excluded from Programmer/Cue recording and
    /// Dynamics lanes.
    pub recordable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeBounds {
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedAttributeDescriptor<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub family: AttributeClass,
    pub value_type: AttributeValueType,
    pub display_unit: Option<&'a str>,
    pub physical_unit: Option<&'a str>,
    pub normalized_bounds: Option<AttributeBounds>,
    pub domain_bounds: Option<AttributeBounds>,
    pub cyclic: bool,
    pub recordable: bool,
    pub built_in: bool,
}

impl ResolvedAttributeDescriptor<'_> {
    pub const fn supports_dynamics(self) -> bool {
        matches!(self.value_type, AttributeValueType::Continuous)
            && self.recordable
            && self.normalized_bounds.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeValueType {
    Continuous,
    Color,
    Indexed,
    Control,
}

pub const ATTRIBUTE_CONFIGURATION_VERSION: u16 = 1;
pub const ENCODER_SLOTS_PER_PAGE: u8 = 6;

/// Compatibility-only built-ins accepted in existing shows but not offered to new fixtures or
/// placed on recommended encoder pages.
pub const RETIRED_BUILT_IN_ATTRIBUTES: &[&str] = &[
    "beam",
    "beam.edge",
    "beam.effect.1",
    "beam.effect.2",
    "control.mode",
    "control.speed",
    "color.cyan",
    "color.cold_white",
    "color.magenta",
    "color.warm_white",
    "color.yellow",
    "media.opacity",
    "media.rotation",
    "media.tint",
    "frost.1",
    "frost.2",
    "pan.continuous",
    "pan.time",
    "position.mode",
    "position.speed",
    "position.time",
    "tilt.time",
    "tilt.continuous",
    "shaper.keystone.x",
    "shaper.keystone.y",
    "strobe",
];

pub fn built_in_attribute_is_retired(attribute: &str) -> bool {
    RETIRED_BUILT_IN_ATTRIBUTES.contains(&attribute)
}

/// Fixture-facing coordinates populated by a compound canonical value. They are known built-ins
/// for profile validation/import, but do not receive independent Programmer encoders or activation
/// groups because the operator authors them through the whole-color control.
pub const PROJECTION_ONLY_BUILT_IN_ATTRIBUTES: &[&str] =
    &["color.hue", "color.saturation", "color.brightness"];

pub fn built_in_attribute_is_projection_only(attribute: &str) -> bool {
    PROJECTION_ONLY_BUILT_IN_ATTRIBUTES.contains(&attribute)
}

/// Recordable built-ins edited through a dedicated semantic surface instead of occupying a
/// permanent encoder slot.
pub const SPECIAL_DIALOG_ONLY_BUILT_IN_ATTRIBUTES: &[&str] =
    &["color", "color.tint", "media.grayscale"];

pub fn built_in_attribute_is_special_dialog_only(attribute: &str) -> bool {
    SPECIAL_DIALOG_ONLY_BUILT_IN_ATTRIBUTES.contains(&attribute)
}

/// The eight fixed programmer tabs. Pages add capacity without changing this hardware-facing
/// vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncoderGroup {
    Intensity,
    Color,
    Position,
    Beam,
    Shapers,
    Focus,
    Control,
    Media,
}

/// Stable, one-based encoder location within one of the fixed programmer tabs.
///
/// Deserialization deliberately remains lossless. Call [`AttributeConfiguration::validate`]
/// before using persisted data so a future migration can inspect an invalid legacy location
/// instead of Serde discarding it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct EncoderPlacement {
    pub group: EncoderGroup,
    pub page: u16,
    pub slot: u8,
}

impl EncoderPlacement {
    pub const fn new(group: EncoderGroup, page: u16, slot: u8) -> Self {
        Self { group, page, slot }
    }

    pub const fn is_valid(self) -> bool {
        self.page > 0 && self.slot > 0 && self.slot <= ENCODER_SLOTS_PER_PAGE
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttributePlacement {
    pub attribute: AttributeKey,
    pub encoder: EncoderPlacement,
    /// Attribute controlled by the encoder's ordinary turn when this attribute is its push-turn
    /// companion. Both values remain independently recordable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_turn_of: Option<AttributeKey>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomAttributeLifecycle {
    #[default]
    Active,
    Retired,
}

/// Show-owned metadata for an operator-authored canonical attribute.
///
/// The stable ID is intentionally independent of the editable label. A retired descriptor
/// remains resolvable in old Programmer, Preset, Cue, and fixture-profile data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomAttributeDescriptor {
    pub id: AttributeKey,
    pub label: String,
    pub value_type: AttributeValueType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_bounds: Option<AttributeBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_bounds: Option<AttributeBounds>,
    #[serde(default)]
    pub cyclic: bool,
    pub recordable: bool,
    #[serde(default)]
    pub lifecycle: CustomAttributeLifecycle,
}

impl CustomAttributeDescriptor {
    /// Generates a collision-resistant stable ID for a newly authored custom descriptor.
    pub fn generated_id() -> AttributeKey {
        AttributeKey(format!("custom.{}", Uuid::new_v4()).into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttributeActivationGroup {
    /// Stable show-local identity; the label may be edited without changing references.
    pub id: String,
    pub label: String,
    pub members: Vec<AttributeKey>,
}

/// Versioned, portable show configuration. Descriptor metadata, presentation, and record
/// activation are separate collections so labels can evolve without rewriting value identities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeConfiguration {
    pub version: u16,
    #[serde(default)]
    pub custom_attributes: Vec<CustomAttributeDescriptor>,
    #[serde(default)]
    pub placements: Vec<AttributePlacement>,
    #[serde(default)]
    pub activation_groups: Vec<AttributeActivationGroup>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AttributeConfigurationError {
    #[error("unsupported attribute configuration version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u16, expected: u16 },
    #[error("custom attribute ID `{0}` must be a bounded lowercase namespaced ID")]
    InvalidCustomId(String),
    #[error("custom attribute `{0}` shadows a built-in attribute")]
    BuiltInShadow(String),
    #[error("duplicate custom attribute ID `{0}`")]
    DuplicateCustomAttribute(String),
    #[error("custom attribute `{0}` requires a non-empty label")]
    EmptyCustomLabel(String),
    #[error("custom attribute `{0}` has invalid scalar bounds")]
    InvalidCustomBounds(String),
    #[error("control attribute `{0}` cannot be recordable")]
    RecordableControl(String),
    #[error("attribute `{0}` has no encoder placement")]
    MissingPlacement(String),
    #[error("attribute `{0}` has more than one encoder placement")]
    DuplicatePlacement(String),
    #[error("attribute `{0}` has an invalid encoder page or slot")]
    InvalidPlacement(String),
    #[error("encoder position {group:?} page {page} slot {slot} is assigned more than once")]
    OccupiedPlacement {
        group: EncoderGroup,
        page: u16,
        slot: u8,
    },
    #[error("encoder placement references unknown attribute `{0}`")]
    UnknownPlacedAttribute(String),
    #[error("activation group ID `{0}` is empty or duplicated")]
    InvalidActivationGroupId(String),
    #[error("activation group `{0}` requires a non-empty label")]
    EmptyActivationGroupLabel(String),
    #[error("activation group `{0}` has no members")]
    EmptyActivationGroup(String),
    #[error("activation group `{group}` repeats attribute `{attribute}`")]
    DuplicateActivationMember { group: String, attribute: String },
    #[error("activation group `{group}` references unknown attribute `{attribute}`")]
    UnknownActivationMember { group: String, attribute: String },
    #[error("non-recordable or control attribute `{0}` cannot be in an activation group")]
    IneligibleActivationMember(String),
    #[error("activation group `{group}` crosses encoder groups at attribute `{attribute}`")]
    CrossEncoderActivationGroup { group: String, attribute: String },
    #[error("recordable attribute `{0}` has no activation group")]
    MissingActivationGroup(String),
    #[error("attribute `{0}` belongs to overlapping activation groups")]
    OverlappingActivationGroup(String),
    #[error("push-turn attribute `{attribute}` references invalid parent `{parent}`")]
    InvalidPushTurnParent { attribute: String, parent: String },
    #[error("encoder `{parent}` has more than one push-turn attribute")]
    DuplicatePushTurnCompanion { parent: String },
    #[error("push-turn pair `{parent}` / `{attribute}` crosses encoder groups")]
    CrossEncoderPushTurn { parent: String, attribute: String },
    #[error(
        "legacy attribute `{legacy}` and canonical attribute `{canonical}` have conflicting encoder placements"
    )]
    CanonicalPlacementConflict { legacy: String, canonical: String },
    #[error(
        "legacy attribute `{legacy}` and canonical attribute `{canonical}` belong to incompatible activation groups"
    )]
    CanonicalActivationGroupConflict { legacy: String, canonical: String },
}

mod configuration;
mod table;
#[cfg(test)]
use configuration::valid_custom_attribute_id;
pub use configuration::{ATTRIBUTE_REGISTRY, attribute_descriptor};
pub use table::{AttributeEntry, AttributeId, AttributeTable};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeKey(pub std::sync::Arc<str>);

/// The two names every head is asked about on every frame, allocated once. Building either from
/// its literal costs a heap allocation, and the render used to do that twice per head per tick.
static INTENSITY: std::sync::LazyLock<AttributeKey> =
    std::sync::LazyLock::new(|| AttributeKey("intensity".into()));
static COLOR: std::sync::LazyLock<AttributeKey> =
    std::sync::LazyLock::new(|| AttributeKey("color".into()));

impl AttributeKey {
    /// The canonical intensity attribute, shared rather than allocated.
    pub fn intensity() -> Self {
        INTENSITY.clone()
    }

    /// The canonical colour attribute, shared rather than allocated.
    pub fn color() -> Self {
        COLOR.clone()
    }

    pub fn is_intensity(&self) -> bool {
        *self.0 == *"intensity" || self.0.ends_with(".intensity")
    }

    pub fn is_position(&self) -> bool {
        *self.0 == *"pan"
            || *self.0 == *"tilt"
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

// @tour rust-by-example:10 Model domain values with an exhaustive enum
// Each variant carries only valid data for that representation. Serde gives TypeScript a
// discriminated wire shape while Rust requires exhaustive matching.

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

/// Versioned canonical identity migration used at fixture and persisted-value boundaries. Source
/// fixture identities remain untouched; only their canonical projection and stored desk/show
/// values use this table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalAttributeTransform {
    Identity,
    InvertNormalized,
}

pub fn canonical_attribute_migration(
    attribute: &AttributeKey,
) -> Option<(AttributeKey, CanonicalAttributeTransform)> {
    canonical_attribute_migration_id(&attribute.0)
        .map(|(canonical, transform)| (AttributeKey(canonical.into()), transform))
}

pub fn canonical_attribute_migration_id(
    attribute: &str,
) -> Option<(&'static str, CanonicalAttributeTransform)> {
    let migration = match attribute {
        "color.cyan" => ("color.red", CanonicalAttributeTransform::InvertNormalized),
        "color.magenta" => ("color.green", CanonicalAttributeTransform::InvertNormalized),
        "color.yellow" => ("color.blue", CanonicalAttributeTransform::InvertNormalized),
        "color.cold_white" => ("color.white", CanonicalAttributeTransform::Identity),
        "color.warm_white" => ("color.amber", CanonicalAttributeTransform::Identity),
        "fixture.mspeed"
        | "fixture.pan_tilt_speed"
        | "fixture.pan_tilt_speed_time"
        | "fixture.pan_tilt_time" => ("position.movement", CanonicalAttributeTransform::Identity),
        "frost" | "frost.1" | "beam.edge" => ("softness", CanonicalAttributeTransform::Identity),
        "media.opacity" => ("intensity", CanonicalAttributeTransform::Identity),
        "media.rotation" => ("position.rotation", CanonicalAttributeTransform::Identity),
        "media.tint" => ("color", CanonicalAttributeTransform::Identity),
        "media.layer.folder" => ("media.folder", CanonicalAttributeTransform::Identity),
        "media.layer.file" => ("media.file", CanonicalAttributeTransform::Identity),
        "media.layer.play.mode" => ("media.play_mode", CanonicalAttributeTransform::Identity),
        "media.layer.scale.x" => ("media.scale.x", CanonicalAttributeTransform::Identity),
        "media.layer.scale.y" => ("media.scale.y", CanonicalAttributeTransform::Identity),
        "media.layer.scaling.mode" => ("media.scaling_mode", CanonicalAttributeTransform::Identity),
        "media.layer.position.x" => ("media.position.x", CanonicalAttributeTransform::Identity),
        "media.layer.position.y" => ("media.position.y", CanonicalAttributeTransform::Identity),
        "media.layer.rotation" => ("position.rotation", CanonicalAttributeTransform::Identity),
        "media.layer.dimmer" | "media.master.master.dimmer" => {
            ("intensity", CanonicalAttributeTransform::Identity)
        }
        "media.layer.volume" | "media.master.master.volume" => {
            ("volume", CanonicalAttributeTransform::Identity)
        }
        "media.layer.cyan" | "media.master.master.cyan" => {
            ("color.red", CanonicalAttributeTransform::InvertNormalized)
        }
        "media.layer.magenta" | "media.master.master.magenta" => {
            ("color.green", CanonicalAttributeTransform::InvertNormalized)
        }
        "media.layer.yellow" | "media.master.master.yellow" => {
            ("color.blue", CanonicalAttributeTransform::InvertNormalized)
        }
        "media.layer.grayscale" => ("media.grayscale", CanonicalAttributeTransform::Identity),
        "media.layer.mask.folder" => ("media.mask.folder", CanonicalAttributeTransform::Identity),
        "media.layer.mask.file" | "media.master.master.mask" => {
            ("media.mask.file", CanonicalAttributeTransform::Identity)
        }
        "media.layer.mask.scale.x" => ("media.mask.scale.x", CanonicalAttributeTransform::Identity),
        "media.layer.mask.scale.y" => ("media.mask.scale.y", CanonicalAttributeTransform::Identity),
        "media.layer.mask.position.x" | "media.master.mask.position.x" => (
            "media.mask.position.x",
            CanonicalAttributeTransform::Identity,
        ),
        "media.layer.mask.position.y" | "media.master.mask.position.y" => (
            "media.mask.position.y",
            CanonicalAttributeTransform::Identity,
        ),
        "media.layer.mask.invert" => ("media.mask.invert", CanonicalAttributeTransform::Identity),
        "media.layer.mask.opacity" => ("media.mask.opacity", CanonicalAttributeTransform::Identity),
        "media.layer.effect.1" => ("media.effect.1", CanonicalAttributeTransform::Identity),
        "media.layer.effect.2" => ("media.effect.2", CanonicalAttributeTransform::Identity),
        "media.layer.effect.3" => ("media.effect.3", CanonicalAttributeTransform::Identity),
        "media.layer.effect.4" => ("media.effect.4", CanonicalAttributeTransform::Identity),
        "media.layer.speed.multiplier" => (
            "media.playback_speed",
            CanonicalAttributeTransform::Identity,
        ),
        "media.layer.playback.bpm" => ("media.playback_bpm", CanonicalAttributeTransform::Identity),
        "media.master.flip.mirror" => ("media.flip_mirror", CanonicalAttributeTransform::Identity),
        "pan.continuous" => ("pan", CanonicalAttributeTransform::Identity),
        "pan.time" | "tilt.time" | "position.time" | "position.speed" => {
            ("position.movement", CanonicalAttributeTransform::Identity)
        }
        "strobe" => ("shutter", CanonicalAttributeTransform::Identity),
        "tilt.continuous" => ("tilt", CanonicalAttributeTransform::Identity),
        _ => return None,
    };
    Some(migration)
}

pub fn transform_canonical_normalized(value: f32, transform: CanonicalAttributeTransform) -> f32 {
    match transform {
        CanonicalAttributeTransform::Identity => value,
        CanonicalAttributeTransform::InvertNormalized => 1.0 - value,
    }
}

pub fn transform_canonical_value(
    value: &mut AttributeValue,
    transform: CanonicalAttributeTransform,
) -> Result<(), &'static str> {
    match (transform, value) {
        (CanonicalAttributeTransform::Identity, _) => Ok(()),
        (CanonicalAttributeTransform::InvertNormalized, AttributeValue::Normalized(value)) => {
            *value = transform_canonical_normalized(
                *value,
                CanonicalAttributeTransform::InvertNormalized,
            );
            Ok(())
        }
        (CanonicalAttributeTransform::InvertNormalized, AttributeValue::Spread(values)) => {
            for value in values {
                *value = transform_canonical_normalized(
                    *value,
                    CanonicalAttributeTransform::InvertNormalized,
                );
            }
            Ok(())
        }
        (CanonicalAttributeTransform::InvertNormalized, _) => {
            Err("inverse canonical migration requires a normalized or spread value")
        }
    }
}

// @tour value-spreading:30 Resolve control points over selection order
// This deterministic anchor and interpolation rule is the semantic oracle shared by live Group
// recall, frozen values, and ordinary ordered selections.

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

/// Hue/saturation picker coordinates (both 0..1) plus brightness, as used by the operator
/// color dialog. Hue is cyclic; saturation and brightness are plain unit scalars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickerColor {
    pub hue: f32,
    pub saturation: f32,
    pub brightness: f32,
}

/// Exact port of the operator color dialog's HSV→RGB conversion so a server-resolved color
/// range reproduces the former client bytes.
pub fn hsv_to_rgb(color: PickerColor) -> [f32; 3] {
    let PickerColor {
        hue,
        saturation,
        brightness,
    } = color;
    let i = (hue * 6.0).floor();
    let f = hue * 6.0 - i;
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - f * saturation);
    let t = brightness * (1.0 - (1.0 - f) * saturation);
    match (i as i32).rem_euclid(6) {
        0 => [brightness, t, p],
        1 => [q, brightness, p],
        2 => [p, brightness, t],
        3 => [p, q, brightness],
        4 => [t, p, brightness],
        _ => [brightness, p, q],
    }
}

/// Resolves the color at `index` of an ordered `count`-strong selection for a color-range
/// gesture. Hue interpolates along `hue_travel` (total signed travel in revolutions, so the
/// long way around and multiple full rainbow cycles are expressible) with wraparound;
/// saturation interpolates linearly; every color uses the uniform `brightness` (the dialog's
/// range semantics). Endpoints are pinned — except for a closed loop (integer non-zero travel
/// back to the start color), where the wheel distributes across `count` slots so a full
/// revolution over N fixtures yields N distinct evenly spaced hues instead of repeating the
/// start.
pub fn color_range_color(
    start: (f32, f32),
    end: (f32, f32),
    hue_travel: f32,
    brightness: f32,
    index: usize,
    count: usize,
) -> PickerColor {
    let closed = count > 1 && hue_travel != 0.0 && hue_travel.fract() == 0.0 && start == end;
    if count <= 1 || (!closed && index + 1 >= count) {
        return PickerColor {
            hue: end.0,
            saturation: end.1,
            brightness,
        };
    }
    let ratio = if closed {
        index as f32 / count as f32
    } else {
        index as f32 / (count - 1) as f32
    };
    PickerColor {
        hue: (start.0 + hue_travel * ratio).rem_euclid(1.0),
        saturation: start.1 + (end.1 - start.1) * ratio,
        brightness,
    }
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
mod tests;
