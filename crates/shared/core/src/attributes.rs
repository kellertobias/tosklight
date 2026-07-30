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
        AttributeKey(format!("custom.{}", Uuid::new_v4()))
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
}

impl AttributeConfiguration {
    pub fn recommended() -> Self {
        let placements = recommended_builtin_placements();
        let mut linked_groups = vec![
            recommended_activation_group(
                "color_mix",
                "Color Mix",
                &[
                    "color",
                    "color.red",
                    "color.green",
                    "color.blue",
                    "color.amber",
                    "color.white",
                    "color.uv",
                    "color.cold_white",
                    "color.warm_white",
                    "color.lime",
                    "color.indigo",
                    "color.mint",
                    "color.temperature",
                    "color.tint",
                ],
            ),
            recommended_activation_group(
                "color_wheel_1",
                "Color Wheel 1",
                &["color.wheel.1", "color.wheel.1.rotation"],
            ),
            recommended_activation_group(
                "color_wheel_2",
                "Color Wheel 2",
                &["color.wheel.2", "color.wheel.2.rotation"],
            ),
            recommended_activation_group(
                "position",
                "Position",
                &["pan", "tilt", "position.rotation"],
            ),
            recommended_activation_group("gobo_1", "Gobo 1", &["gobo.1", "gobo.1.rotation"]),
            recommended_activation_group("gobo_2", "Gobo 2", &["gobo.2", "gobo.2.rotation"]),
            recommended_activation_group(
                "media_source",
                "Media Source",
                &["media.folder", "media.file"],
            ),
            recommended_activation_group(
                "media_mask_source",
                "Media Mask Source",
                &["media.mask.folder", "media.mask.file"],
            ),
            recommended_activation_group(
                "shapers",
                "Shapers",
                &[
                    "iris",
                    "shaper.blade.1.position",
                    "shaper.blade.1.angle",
                    "shaper.blade.2.position",
                    "shaper.blade.2.angle",
                    "shaper.rotation",
                    "shaper.blade.3.position",
                    "shaper.blade.3.angle",
                    "shaper.blade.4.position",
                    "shaper.blade.4.angle",
                    "shaper.keystone.x",
                    "shaper.keystone.y",
                ],
            ),
        ];
        let linked_members = linked_groups
            .iter()
            .flat_map(|group| group.members.iter().map(|member| member.0.clone()))
            .collect::<HashSet<_>>();
        linked_groups.extend(
            ATTRIBUTE_REGISTRY
                .iter()
                .filter(|descriptor| descriptor.recordable)
                .filter(|descriptor| !linked_members.contains(descriptor.id))
                .map(|descriptor| {
                    recommended_activation_group(descriptor.id, descriptor.label, &[descriptor.id])
                }),
        );
        Self {
            version: ATTRIBUTE_CONFIGURATION_VERSION,
            custom_attributes: Vec::new(),
            placements,
            activation_groups: linked_groups,
        }
    }

    pub fn validate(&self) -> Result<(), AttributeConfigurationError> {
        if self.version != ATTRIBUTE_CONFIGURATION_VERSION {
            return Err(AttributeConfigurationError::UnsupportedVersion {
                actual: self.version,
                expected: ATTRIBUTE_CONFIGURATION_VERSION,
            });
        }
        let mut descriptors = ATTRIBUTE_REGISTRY
            .iter()
            .map(|descriptor| {
                (
                    descriptor.id,
                    (descriptor.value_type, descriptor.recordable),
                )
            })
            .collect::<HashMap<_, _>>();
        let built_in_ids = descriptors.keys().copied().collect::<HashSet<_>>();
        for descriptor in &self.custom_attributes {
            validate_custom_descriptor(descriptor, &built_in_ids)?;
            if descriptors
                .insert(
                    descriptor.id.0.as_str(),
                    (descriptor.value_type, descriptor.recordable),
                )
                .is_some()
            {
                return Err(AttributeConfigurationError::DuplicateCustomAttribute(
                    descriptor.id.0.clone(),
                ));
            }
        }

        let mut placements = HashMap::new();
        let mut occupied = HashSet::new();
        for placement in &self.placements {
            let id = placement.attribute.0.as_str();
            if !descriptors.contains_key(id) {
                return Err(AttributeConfigurationError::UnknownPlacedAttribute(
                    id.to_owned(),
                ));
            }
            if !placement.encoder.is_valid() {
                return Err(AttributeConfigurationError::InvalidPlacement(id.to_owned()));
            }
            if placements.insert(id, placement.encoder).is_some() {
                return Err(AttributeConfigurationError::DuplicatePlacement(
                    id.to_owned(),
                ));
            }
            if !occupied.insert(placement.encoder) {
                return Err(AttributeConfigurationError::OccupiedPlacement {
                    group: placement.encoder.group,
                    page: placement.encoder.page,
                    slot: placement.encoder.slot,
                });
            }
        }
        for id in descriptors.keys() {
            if !placements.contains_key(id) {
                return Err(AttributeConfigurationError::MissingPlacement(
                    (*id).to_owned(),
                ));
            }
        }

        let mut group_ids = HashSet::new();
        let mut activated = HashSet::new();
        for group in &self.activation_groups {
            if group.id.trim().is_empty() || !group_ids.insert(group.id.as_str()) {
                return Err(AttributeConfigurationError::InvalidActivationGroupId(
                    group.id.clone(),
                ));
            }
            if group.label.trim().is_empty() {
                return Err(AttributeConfigurationError::EmptyActivationGroupLabel(
                    group.id.clone(),
                ));
            }
            if group.members.is_empty() {
                return Err(AttributeConfigurationError::EmptyActivationGroup(
                    group.id.clone(),
                ));
            }
            let mut members = HashSet::new();
            let mut encoder_group = None;
            for member in &group.members {
                let id = member.0.as_str();
                if !members.insert(id) {
                    return Err(AttributeConfigurationError::DuplicateActivationMember {
                        group: group.id.clone(),
                        attribute: id.to_owned(),
                    });
                }
                let Some((value_type, recordable)) = descriptors.get(id).copied() else {
                    return Err(AttributeConfigurationError::UnknownActivationMember {
                        group: group.id.clone(),
                        attribute: id.to_owned(),
                    });
                };
                if !recordable || value_type == AttributeValueType::Control {
                    return Err(AttributeConfigurationError::IneligibleActivationMember(
                        id.to_owned(),
                    ));
                }
                let member_encoder_group = placements
                    .get(id)
                    .expect("every descriptor placement was validated")
                    .group;
                if encoder_group
                    .replace(member_encoder_group)
                    .is_some_and(|expected| expected != member_encoder_group)
                {
                    return Err(AttributeConfigurationError::CrossEncoderActivationGroup {
                        group: group.id.clone(),
                        attribute: id.to_owned(),
                    });
                }
                if !activated.insert(id) {
                    return Err(AttributeConfigurationError::OverlappingActivationGroup(
                        id.to_owned(),
                    ));
                }
            }
        }
        for (id, (value_type, recordable)) in descriptors {
            if recordable && value_type != AttributeValueType::Control && !activated.contains(id) {
                return Err(AttributeConfigurationError::MissingActivationGroup(
                    id.to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub fn placement_for(&self, attribute: &AttributeKey) -> Option<EncoderPlacement> {
        self.placements
            .iter()
            .find(|placement| placement.attribute == *attribute)
            .map(|placement| placement.encoder)
    }

    pub fn activation_group_for(
        &self,
        attribute: &AttributeKey,
    ) -> Option<&AttributeActivationGroup> {
        self.activation_groups
            .iter()
            .find(|group| group.members.contains(attribute))
    }

    /// Symmetric linked-member lookup in the stable order authored for each activation group.
    /// Single-member groups intentionally produce an empty link list.
    pub fn activation_links(&self) -> HashMap<AttributeKey, Vec<AttributeKey>> {
        self.activation_groups
            .iter()
            .flat_map(|group| {
                group.members.iter().cloned().map(|member| {
                    let linked = group
                        .members
                        .iter()
                        .filter(|candidate| **candidate != member)
                        .cloned()
                        .collect();
                    (member, linked)
                })
            })
            .collect()
    }
}

impl Default for AttributeConfiguration {
    fn default() -> Self {
        Self::recommended()
    }
}

fn validate_custom_descriptor(
    descriptor: &CustomAttributeDescriptor,
    built_in_ids: &HashSet<&str>,
) -> Result<(), AttributeConfigurationError> {
    let id = descriptor.id.0.as_str();
    if built_in_ids.contains(id) {
        return Err(AttributeConfigurationError::BuiltInShadow(id.to_owned()));
    }
    if !valid_custom_attribute_id(id) {
        return Err(AttributeConfigurationError::InvalidCustomId(id.to_owned()));
    }
    if descriptor.label.trim().is_empty() {
        return Err(AttributeConfigurationError::EmptyCustomLabel(id.to_owned()));
    }
    if descriptor.value_type == AttributeValueType::Control && descriptor.recordable {
        return Err(AttributeConfigurationError::RecordableControl(
            id.to_owned(),
        ));
    }
    if descriptor
        .normalized_bounds
        .into_iter()
        .chain(descriptor.domain_bounds)
        .any(|bounds| {
            !bounds.min.is_finite() || !bounds.max.is_finite() || bounds.min >= bounds.max
        })
        || (descriptor.normalized_bounds.is_some()
            && descriptor.value_type != AttributeValueType::Continuous)
    {
        return Err(AttributeConfigurationError::InvalidCustomBounds(
            id.to_owned(),
        ));
    }
    Ok(())
}

fn valid_custom_attribute_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.split('.').count() >= 2
        && id.split('.').all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                })
        })
}

fn recommended_activation_group(
    id: &str,
    label: &str,
    members: &[&str],
) -> AttributeActivationGroup {
    AttributeActivationGroup {
        id: id.to_owned(),
        label: label.to_owned(),
        members: members
            .iter()
            .map(|member| AttributeKey((*member).to_owned()))
            .collect(),
    }
}

fn recommended_builtin_placements() -> Vec<AttributePlacement> {
    use EncoderGroup::{Beam, Color, Control, Focus, Intensity, Media, Position, Shapers};

    [
        ("intensity", Intensity, 1, 1),
        ("shutter", Intensity, 1, 2),
        ("strobe", Intensity, 1, 3),
        ("volume", Intensity, 1, 4),
        ("color.red", Color, 1, 1),
        ("color.green", Color, 1, 2),
        ("color.blue", Color, 1, 3),
        ("color.white", Color, 1, 4),
        ("color.amber", Color, 1, 5),
        ("color.uv", Color, 1, 6),
        ("color.cold_white", Color, 2, 1),
        ("color.warm_white", Color, 2, 2),
        ("color.lime", Color, 2, 3),
        ("color.indigo", Color, 2, 4),
        ("color.mint", Color, 2, 5),
        ("color.temperature", Color, 2, 6),
        ("color.tint", Color, 3, 1),
        ("color", Color, 3, 2),
        ("color.wheel.1", Color, 3, 3),
        ("color.wheel.1.rotation", Color, 3, 4),
        ("color.wheel.2", Color, 3, 5),
        ("color.wheel.2.rotation", Color, 3, 6),
        // Legacy canonical CMY IDs remain distinct and lossless until the explicit compatible
        // CMY-to-RGB show migration. They are not linked into the recommended Color Mix group.
        ("color.cyan", Color, 4, 1),
        ("color.magenta", Color, 4, 2),
        ("color.yellow", Color, 4, 3),
        ("pan", Position, 1, 1),
        ("tilt", Position, 1, 2),
        ("pan.continuous", Position, 1, 3),
        ("tilt.continuous", Position, 1, 4),
        ("pan.time", Position, 1, 5),
        ("tilt.time", Position, 1, 6),
        ("position.speed", Position, 2, 1),
        ("position.mode", Position, 2, 2),
        ("position.rotation", Position, 2, 3),
        ("gobo.1", Beam, 1, 1),
        ("gobo.1.rotation", Beam, 1, 2),
        ("gobo.2", Beam, 1, 3),
        ("gobo.2.rotation", Beam, 1, 4),
        ("prism.1", Beam, 1, 5),
        ("prism.1.rotation", Beam, 1, 6),
        ("prism.2", Beam, 2, 1),
        ("prism.2.rotation", Beam, 2, 2),
        ("animation.1", Beam, 2, 3),
        ("animation.1.rotation", Beam, 2, 4),
        ("beam.effect.1", Beam, 2, 5),
        ("beam.effect.2", Beam, 2, 6),
        ("beam", Beam, 3, 1),
        ("iris", Shapers, 1, 1),
        ("shaper.blade.1.position", Shapers, 1, 2),
        ("shaper.blade.1.angle", Shapers, 1, 3),
        ("shaper.blade.2.position", Shapers, 1, 4),
        ("shaper.blade.2.angle", Shapers, 1, 5),
        ("shaper.rotation", Shapers, 1, 6),
        ("shaper.blade.3.position", Shapers, 2, 1),
        ("shaper.blade.3.angle", Shapers, 2, 2),
        ("shaper.blade.4.position", Shapers, 2, 3),
        ("shaper.blade.4.angle", Shapers, 2, 4),
        ("shaper.keystone.x", Shapers, 2, 5),
        ("shaper.keystone.y", Shapers, 2, 6),
        ("focus", Focus, 1, 1),
        ("zoom", Focus, 1, 2),
        ("frost.1", Focus, 1, 3),
        ("frost.2", Focus, 1, 4),
        ("beam.edge", Focus, 1, 5),
        ("control.mode", Control, 1, 1),
        ("control.speed", Control, 1, 2),
        ("control", Control, 2, 1),
        ("media.folder", Media, 1, 1),
        ("media.file", Media, 1, 2),
        ("media.mask.folder", Media, 1, 3),
        ("media.mask.file", Media, 1, 4),
        ("media.opacity", Media, 1, 5),
        ("media.tint", Media, 1, 6),
        ("media.play_mode", Media, 2, 1),
        ("media.playback_speed", Media, 2, 2),
        ("media.playback_bpm", Media, 2, 3),
        ("media.grayscale", Media, 2, 4),
        ("media.scaling_mode", Media, 2, 5),
        ("media.rotation", Media, 2, 6),
        ("media.position.x", Media, 3, 1),
        ("media.position.y", Media, 3, 2),
        ("media.scale.x", Media, 3, 3),
        ("media.scale.y", Media, 3, 4),
        ("media.mask.opacity", Media, 3, 5),
        ("media.mask.invert", Media, 3, 6),
        ("media.effect.1", Media, 4, 1),
        ("media.effect.2", Media, 4, 2),
        ("media.effect.3", Media, 4, 3),
        ("media.effect.4", Media, 4, 4),
    ]
    .into_iter()
    .map(|(id, group, page, slot)| AttributePlacement {
        attribute: AttributeKey(id.to_owned()),
        encoder: EncoderPlacement::new(group, page, slot),
    })
    .collect()
}

/// Built-in attribute registry. Custom attributes remain valid and use their persisted identifier
/// as the operator label until a desk extension supplies richer metadata.
pub const ATTRIBUTE_REGISTRY: &[AttributeDescriptor] = &[
    continuous(
        "intensity",
        "Intensity",
        AttributeClass::Intensity,
        "percent",
    ),
    indexed("shutter", "Shutter", AttributeClass::Intensity),
    continuous("strobe", "Strobe", AttributeClass::Intensity, "hz"),
    continuous("volume", "Volume", AttributeClass::Intensity, "percent"),
    color("color", "Color", AttributeClass::Color),
    continuous("color.red", "Red", AttributeClass::Color, "percent"),
    continuous("color.green", "Green", AttributeClass::Color, "percent"),
    continuous("color.blue", "Blue", AttributeClass::Color, "percent"),
    continuous("color.cyan", "Cyan", AttributeClass::Color, "percent"),
    continuous("color.magenta", "Magenta", AttributeClass::Color, "percent"),
    continuous("color.yellow", "Yellow", AttributeClass::Color, "percent"),
    continuous("color.amber", "Amber", AttributeClass::Color, "percent"),
    continuous("color.white", "White", AttributeClass::Color, "percent"),
    continuous("color.uv", "UV", AttributeClass::Color, "percent"),
    continuous(
        "color.cold_white",
        "Cold White",
        AttributeClass::Color,
        "percent",
    ),
    continuous(
        "color.warm_white",
        "Warm White",
        AttributeClass::Color,
        "percent",
    ),
    continuous("color.lime", "Lime", AttributeClass::Color, "percent"),
    continuous("color.indigo", "Indigo", AttributeClass::Color, "percent"),
    continuous("color.mint", "Mint", AttributeClass::Color, "percent"),
    continuous(
        "color.temperature",
        "Color Temperature",
        AttributeClass::Color,
        "K",
    ),
    continuous("color.tint", "Tint", AttributeClass::Color, "percent"),
    indexed("color.wheel.1", "Color Wheel 1", AttributeClass::Color),
    continuous(
        "color.wheel.1.rotation",
        "Color Wheel 1 Rotation",
        AttributeClass::Color,
        "percent",
    ),
    indexed("color.wheel.2", "Color Wheel 2", AttributeClass::Color),
    continuous(
        "color.wheel.2.rotation",
        "Color Wheel 2 Rotation",
        AttributeClass::Color,
        "percent",
    ),
    continuous("pan", "Pan", AttributeClass::Position, "deg"),
    continuous("tilt", "Tilt", AttributeClass::Position, "deg"),
    continuous(
        "pan.continuous",
        "Continuous Pan",
        AttributeClass::Position,
        "percent",
    ),
    continuous(
        "tilt.continuous",
        "Continuous Tilt",
        AttributeClass::Position,
        "percent",
    ),
    continuous("pan.time", "Pan Time", AttributeClass::Position, "s"),
    continuous("tilt.time", "Tilt Time", AttributeClass::Position, "s"),
    continuous(
        "position.speed",
        "Position Speed",
        AttributeClass::Position,
        "percent",
    ),
    indexed("position.mode", "Position Mode", AttributeClass::Position),
    cyclic_continuous(
        "position.rotation",
        "Head Rotation",
        AttributeClass::Position,
        "deg",
    ),
    indexed("gobo.1", "Gobo 1", AttributeClass::Beam),
    continuous(
        "gobo.1.rotation",
        "Gobo 1 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("gobo.2", "Gobo 2", AttributeClass::Beam),
    continuous(
        "gobo.2.rotation",
        "Gobo 2 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("prism.1", "Prism 1", AttributeClass::Beam),
    continuous(
        "prism.1.rotation",
        "Prism 1 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("prism.2", "Prism 2", AttributeClass::Beam),
    continuous(
        "prism.2.rotation",
        "Prism 2 Rotation",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("animation.1", "Animation Wheel 1", AttributeClass::Beam),
    continuous(
        "animation.1.rotation",
        "Animation Rotation 1",
        AttributeClass::Beam,
        "percent",
    ),
    indexed("beam.effect.1", "Beam Effect 1", AttributeClass::Beam),
    indexed("beam.effect.2", "Beam Effect 2", AttributeClass::Beam),
    continuous("beam", "Beam", AttributeClass::Beam, "percent"),
    continuous("iris", "Iris", AttributeClass::Shapers, "percent"),
    continuous(
        "shaper.blade.1.position",
        "Blade 1 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.1.angle",
        "Blade 1 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.blade.2.position",
        "Blade 2 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.2.angle",
        "Blade 2 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    cyclic_continuous(
        "shaper.rotation",
        "Shaper Rotation",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.blade.3.position",
        "Blade 3 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.3.angle",
        "Blade 3 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.blade.4.position",
        "Blade 4 Position",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.blade.4.angle",
        "Blade 4 Angle",
        AttributeClass::Shapers,
        "deg",
    ),
    continuous(
        "shaper.keystone.x",
        "Keystone X",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous(
        "shaper.keystone.y",
        "Keystone Y",
        AttributeClass::Shapers,
        "percent",
    ),
    continuous("focus", "Focus", AttributeClass::Focus, "percent"),
    continuous("zoom", "Zoom", AttributeClass::Focus, "deg"),
    continuous("frost.1", "Frost 1", AttributeClass::Focus, "percent"),
    continuous("frost.2", "Frost 2", AttributeClass::Focus, "percent"),
    continuous("beam.edge", "Beam Edge", AttributeClass::Focus, "percent"),
    indexed("control.mode", "Fixture Mode", AttributeClass::Control),
    continuous(
        "control.speed",
        "Fixture Control Speed",
        AttributeClass::Control,
        "percent",
    ),
    control("control", "Control", AttributeClass::Control),
    indexed("media.folder", "Media Folder", AttributeClass::Media),
    indexed("media.file", "Media File", AttributeClass::Media),
    indexed("media.mask.folder", "Mask Folder", AttributeClass::Media),
    indexed("media.mask.file", "Mask File", AttributeClass::Media),
    continuous(
        "media.opacity",
        "Layer Opacity",
        AttributeClass::Media,
        "percent",
    ),
    color("media.tint", "Layer Tint", AttributeClass::Media),
    indexed("media.play_mode", "Play Mode", AttributeClass::Media),
    continuous(
        "media.playback_speed",
        "Playback Speed",
        AttributeClass::Media,
        "percent",
    ),
    continuous(
        "media.playback_bpm",
        "Playback BPM",
        AttributeClass::Media,
        "bpm",
    ),
    continuous(
        "media.grayscale",
        "Grayscale",
        AttributeClass::Media,
        "percent",
    ),
    indexed("media.scaling_mode", "Scaling Mode", AttributeClass::Media),
    cyclic_continuous(
        "media.rotation",
        "Layer Rotation",
        AttributeClass::Media,
        "deg",
    ),
    continuous(
        "media.position.x",
        "Position X",
        AttributeClass::Media,
        "percent",
    ),
    continuous(
        "media.position.y",
        "Position Y",
        AttributeClass::Media,
        "percent",
    ),
    continuous("media.scale.x", "Scale X", AttributeClass::Media, "percent"),
    continuous("media.scale.y", "Scale Y", AttributeClass::Media, "percent"),
    continuous(
        "media.mask.opacity",
        "Mask Opacity",
        AttributeClass::Media,
        "percent",
    ),
    indexed("media.mask.invert", "Invert Mask", AttributeClass::Media),
    indexed("media.effect.1", "Media Effect 1", AttributeClass::Media),
    indexed("media.effect.2", "Media Effect 2", AttributeClass::Media),
    indexed("media.effect.3", "Media Effect 3", AttributeClass::Media),
    indexed("media.effect.4", "Media Effect 4", AttributeClass::Media),
];

const fn continuous(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    unit: &'static str,
) -> AttributeDescriptor {
    descriptor(
        id,
        label,
        family,
        AttributeValueType::Continuous,
        Some(unit),
    )
}

const fn cyclic_continuous(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    unit: &'static str,
) -> AttributeDescriptor {
    let mut result = continuous(id, label, family, unit);
    result.cyclic = true;
    result
}

const fn indexed(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
) -> AttributeDescriptor {
    descriptor(id, label, family, AttributeValueType::Indexed, None)
}

const fn color(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
) -> AttributeDescriptor {
    descriptor(id, label, family, AttributeValueType::Color, None)
}

const fn control(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
) -> AttributeDescriptor {
    descriptor(id, label, family, AttributeValueType::Control, None)
}

const fn descriptor(
    id: &'static str,
    label: &'static str,
    family: AttributeClass,
    value_type: AttributeValueType,
    default_unit: Option<&'static str>,
) -> AttributeDescriptor {
    let normalized_bounds = match value_type {
        AttributeValueType::Continuous => Some(AttributeBounds { min: 0.0, max: 1.0 }),
        AttributeValueType::Color | AttributeValueType::Indexed | AttributeValueType::Control => {
            None
        }
    };
    AttributeDescriptor {
        id,
        label,
        family,
        value_type,
        default_unit,
        display_unit: default_unit,
        physical_unit: default_unit,
        normalized_bounds,
        domain_bounds: None,
        cyclic: false,
        recordable: !matches!(value_type, AttributeValueType::Control),
    }
}

pub fn attribute_descriptor<'a>(key: &'a AttributeKey) -> ResolvedAttributeDescriptor<'a> {
    ATTRIBUTE_REGISTRY
        .iter()
        .find(|descriptor| descriptor.id == key.0)
        .map(resolved_descriptor)
        .unwrap_or_else(|| custom_descriptor(key))
}

const fn resolved_descriptor(
    descriptor: &'static AttributeDescriptor,
) -> ResolvedAttributeDescriptor<'static> {
    ResolvedAttributeDescriptor {
        id: descriptor.id,
        label: descriptor.label,
        family: descriptor.family,
        value_type: descriptor.value_type,
        display_unit: descriptor.display_unit,
        physical_unit: descriptor.physical_unit,
        normalized_bounds: descriptor.normalized_bounds,
        domain_bounds: descriptor.domain_bounds,
        cyclic: descriptor.cyclic,
        recordable: descriptor.recordable,
        built_in: true,
    }
}

fn custom_descriptor(key: &AttributeKey) -> ResolvedAttributeDescriptor<'_> {
    ResolvedAttributeDescriptor {
        id: &key.0,
        label: &key.0,
        family: AttributeClass::Custom,
        value_type: AttributeValueType::Continuous,
        display_unit: None,
        physical_unit: None,
        normalized_bounds: None,
        domain_bounds: None,
        cyclic: false,
        recordable: false,
        built_in: false,
    }
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
mod color_range_tests {
    use super::{PickerColor, color_range_color, hsv_to_rgb};

    fn hues(start: (f32, f32), end: (f32, f32), travel: f32, count: usize) -> Vec<f32> {
        (0..count)
            .map(|index| {
                let color = color_range_color(start, end, travel, 1.0, index, count);
                (color.hue * 360.0 * 10.0).round() / 10.0
            })
            .collect()
    }

    #[test]
    fn one_full_revolution_from_red_distributes_the_wheel_without_repeating() {
        // Maintainer-pinned: 3 fixtures, red → red with one revolution → 0°, 120°, 240°.
        assert_eq!(hues((0.0, 1.0), (0.0, 1.0), 1.0, 3), [0.0, 120.0, 240.0]);
        // Two revolutions cover the wheel twice across six fixtures.
        assert_eq!(
            hues((0.0, 1.0), (0.0, 1.0), 2.0, 6),
            [0.0, 120.0, 240.0, 0.0, 120.0, 240.0]
        );
        // Reverse direction winds the other way.
        assert_eq!(hues((0.0, 1.0), (0.0, 1.0), -1.0, 3), [0.0, 240.0, 120.0]);
    }

    #[test]
    fn open_arcs_pin_both_endpoints_and_wrap_through_the_seam() {
        // Straight short-way range keeps the former client semantics: endpoints exact.
        assert_eq!(
            hues((0.0, 1.0), (2.0 / 3.0, 1.0), 2.0 / 3.0, 3),
            [0.0, 120.0, 240.0]
        );
        // The long way around from red to blue passes through the wrap seam.
        let long_way = hues((0.0, 1.0), (2.0 / 3.0, 1.0), -1.0 / 3.0, 3);
        assert_eq!(long_way, [0.0, 300.0, 240.0]);
        // Saturation interpolates linearly while brightness stays uniform.
        let middle = color_range_color((0.0, 0.2), (0.5, 0.8), 0.5, 0.4, 1, 3);
        assert_eq!(middle.saturation, 0.5);
        assert_eq!(middle.brightness, 0.4);
    }

    #[test]
    fn single_and_zero_counts_resolve_to_the_end_color() {
        let single = color_range_color((0.1, 0.3), (0.6, 0.9), 0.5, 0.7, 0, 1);
        assert_eq!((single.hue, single.saturation), (0.6, 0.9));
        let none = color_range_color((0.1, 0.3), (0.6, 0.9), 0.5, 0.7, 0, 0);
        assert_eq!((none.hue, none.saturation), (0.6, 0.9));
    }

    #[test]
    fn hsv_conversion_matches_the_former_client_table() {
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 0.0,
                saturation: 1.0,
                brightness: 1.0
            }),
            [1.0, 0.0, 0.0]
        );
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 1.0 / 3.0,
                saturation: 1.0,
                brightness: 1.0
            }),
            [0.0, 1.0, 0.0]
        );
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 2.0 / 3.0,
                saturation: 1.0,
                brightness: 1.0
            }),
            [0.0, 0.0, 1.0]
        );
        assert_eq!(
            hsv_to_rgb(PickerColor {
                hue: 0.0,
                saturation: 0.0,
                brightness: 0.5
            }),
            [0.5, 0.5, 0.5]
        );
    }
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

#[cfg(test)]
mod attribute_registry_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn built_in_ids_are_unique_and_only_recordable_continuous_values_are_scalar_lanes() {
        let mut ids = HashSet::new();
        for descriptor in ATTRIBUTE_REGISTRY {
            assert!(ids.insert(descriptor.id), "duplicate {}", descriptor.id);
            assert!(!descriptor.id.trim().is_empty());
            assert_eq!(descriptor.default_unit, descriptor.display_unit);
            if descriptor.normalized_bounds.is_some() {
                assert_eq!(descriptor.value_type, AttributeValueType::Continuous);
                assert!(descriptor.recordable);
            }
            if let Some(bounds) = descriptor.normalized_bounds {
                assert!(bounds.min.is_finite());
                assert!(bounds.max.is_finite());
                assert!(bounds.min < bounds.max);
            }
        }
    }

    #[test]
    fn unknown_identity_is_retained_but_is_not_assumed_safe_for_dynamics() {
        let key = AttributeKey("vendor.custom.zoomish".into());
        let descriptor = attribute_descriptor(&key);
        assert_eq!(descriptor.id, key.0);
        assert_eq!(descriptor.label, key.0);
        assert_eq!(descriptor.family, AttributeClass::Custom);
        assert!(!descriptor.built_in);
        assert!(!descriptor.recordable);
        assert_eq!(descriptor.normalized_bounds, None);
    }

    #[test]
    fn recommended_configuration_is_complete_stable_and_symmetric() {
        let configuration = AttributeConfiguration::recommended();
        configuration.validate().unwrap();
        assert_eq!(configuration.version, ATTRIBUTE_CONFIGURATION_VERSION);
        assert_eq!(configuration.placements.len(), ATTRIBUTE_REGISTRY.len());
        assert_eq!(
            configuration.placement_for(&AttributeKey("color.red".into())),
            Some(EncoderPlacement::new(EncoderGroup::Color, 1, 1))
        );
        assert_eq!(
            configuration.placement_for(&AttributeKey("iris".into())),
            Some(EncoderPlacement::new(EncoderGroup::Shapers, 1, 1))
        );
        assert_eq!(
            configuration.placement_for(&AttributeKey("media.folder".into())),
            Some(EncoderPlacement::new(EncoderGroup::Media, 1, 1))
        );
        assert!(
            attribute_descriptor(&AttributeKey("media.rotation".into())).cyclic,
            "canonical rotation metadata must wrap instead of clamp"
        );
        assert_eq!(
            configuration
                .placements
                .iter()
                .map(|placement| placement.encoder.group)
                .collect::<HashSet<_>>(),
            HashSet::from([
                EncoderGroup::Intensity,
                EncoderGroup::Color,
                EncoderGroup::Position,
                EncoderGroup::Beam,
                EncoderGroup::Shapers,
                EncoderGroup::Focus,
                EncoderGroup::Control,
                EncoderGroup::Media,
            ])
        );
        assert_eq!(
            configuration
                .activation_group_for(&AttributeKey("color.wheel.1".into()))
                .unwrap()
                .members,
            [
                AttributeKey("color.wheel.1".into()),
                AttributeKey("color.wheel.1.rotation".into())
            ]
        );
        let links = configuration.activation_links();
        assert_eq!(
            links[&AttributeKey("pan".into())],
            [
                AttributeKey("tilt".into()),
                AttributeKey("position.rotation".into())
            ]
        );
        assert_eq!(
            links[&AttributeKey("tilt".into())],
            [
                AttributeKey("pan".into()),
                AttributeKey("position.rotation".into())
            ]
        );
        assert_eq!(
            links[&AttributeKey("media.folder".into())],
            [AttributeKey("media.file".into())]
        );
        assert!(links[&AttributeKey("intensity".into())].is_empty());
        assert!(!links.contains_key(&AttributeKey("control".into())));
    }

    #[test]
    fn encoder_group_vocabulary_is_exact_and_configuration_is_clone_stable() {
        let groups = [
            EncoderGroup::Intensity,
            EncoderGroup::Color,
            EncoderGroup::Position,
            EncoderGroup::Beam,
            EncoderGroup::Shapers,
            EncoderGroup::Focus,
            EncoderGroup::Control,
            EncoderGroup::Media,
        ];
        assert_eq!(groups.len(), 8);
        let configuration = AttributeConfiguration::recommended();
        assert_eq!(configuration.clone(), configuration);
    }

    fn custom_descriptor(
        id: &str,
        value_type: AttributeValueType,
        recordable: bool,
    ) -> CustomAttributeDescriptor {
        CustomAttributeDescriptor {
            id: AttributeKey(id.into()),
            label: "Custom Feature".into(),
            value_type,
            display_unit: Some("percent".into()),
            physical_unit: None,
            normalized_bounds: (value_type == AttributeValueType::Continuous)
                .then_some(AttributeBounds { min: 0.0, max: 1.0 }),
            domain_bounds: None,
            cyclic: false,
            recordable,
            lifecycle: CustomAttributeLifecycle::Active,
        }
    }

    fn add_custom(
        configuration: &mut AttributeConfiguration,
        descriptor: CustomAttributeDescriptor,
        encoder: EncoderPlacement,
    ) {
        let id = descriptor.id.clone();
        configuration.custom_attributes.push(descriptor);
        configuration.placements.push(AttributePlacement {
            attribute: id.clone(),
            encoder,
        });
        if configuration
            .custom_attributes
            .last()
            .is_some_and(|descriptor| descriptor.recordable)
        {
            configuration
                .activation_groups
                .push(AttributeActivationGroup {
                    id: format!("activation.{}", id.0),
                    label: "Custom Feature".into(),
                    members: vec![id],
                });
        }
    }

    #[test]
    fn custom_metadata_accepts_namespaced_ids_and_retirement_without_changing_identity() {
        let mut configuration = AttributeConfiguration::recommended();
        let mut descriptor =
            custom_descriptor("vendor.feature-name", AttributeValueType::Continuous, true);
        descriptor.lifecycle = CustomAttributeLifecycle::Retired;
        add_custom(
            &mut configuration,
            descriptor,
            EncoderPlacement::new(EncoderGroup::Beam, 9, 1),
        );
        configuration.validate().unwrap();
        assert_eq!(
            configuration.custom_attributes[0].id,
            AttributeKey("vendor.feature-name".into())
        );
        let generated = CustomAttributeDescriptor::generated_id();
        assert!(generated.0.starts_with("custom."));
        assert!(valid_custom_attribute_id(&generated.0));
    }

    #[test]
    fn custom_ids_cannot_shadow_built_ins_or_use_unstable_syntax() {
        let mut shadow = AttributeConfiguration::recommended();
        add_custom(
            &mut shadow,
            custom_descriptor("color.red", AttributeValueType::Continuous, true),
            EncoderPlacement::new(EncoderGroup::Color, 9, 1),
        );
        assert_eq!(
            shadow.validate(),
            Err(AttributeConfigurationError::BuiltInShadow(
                "color.red".into()
            ))
        );

        let mut invalid = AttributeConfiguration::recommended();
        add_custom(
            &mut invalid,
            custom_descriptor("Feature", AttributeValueType::Continuous, true),
            EncoderPlacement::new(EncoderGroup::Beam, 9, 1),
        );
        assert_eq!(
            invalid.validate(),
            Err(AttributeConfigurationError::InvalidCustomId(
                "Feature".into()
            ))
        );
    }

    #[test]
    fn placements_are_one_based_six_slot_unique_and_complete() {
        let mut invalid_slot = AttributeConfiguration::recommended();
        invalid_slot.placements[0].encoder.slot = ENCODER_SLOTS_PER_PAGE + 1;
        assert!(matches!(
            invalid_slot.validate(),
            Err(AttributeConfigurationError::InvalidPlacement(_))
        ));

        let mut occupied = AttributeConfiguration::recommended();
        occupied.placements[1].encoder = occupied.placements[0].encoder;
        assert!(matches!(
            occupied.validate(),
            Err(AttributeConfigurationError::OccupiedPlacement { .. })
        ));

        let mut duplicate = AttributeConfiguration::recommended();
        duplicate.placements.push(duplicate.placements[0].clone());
        assert!(matches!(
            duplicate.validate(),
            Err(AttributeConfigurationError::DuplicatePlacement(_))
        ));

        let mut missing = AttributeConfiguration::recommended();
        missing.placements.pop();
        assert!(matches!(
            missing.validate(),
            Err(AttributeConfigurationError::MissingPlacement(_))
        ));
    }

    #[test]
    fn activation_groups_are_exclusive_recordable_and_within_one_encoder_group() {
        let mut overlap = AttributeConfiguration::recommended();
        overlap.activation_groups.push(AttributeActivationGroup {
            id: "second.intensity".into(),
            label: "Second Intensity".into(),
            members: vec![AttributeKey("intensity".into())],
        });
        assert_eq!(
            overlap.validate(),
            Err(AttributeConfigurationError::OverlappingActivationGroup(
                "intensity".into()
            ))
        );

        let mut cross_group = AttributeConfiguration::recommended();
        let position = cross_group
            .activation_groups
            .iter_mut()
            .find(|group| group.id == "position")
            .unwrap();
        position.members.push(AttributeKey("color.red".into()));
        assert!(matches!(
            cross_group.validate(),
            Err(AttributeConfigurationError::CrossEncoderActivationGroup { .. })
        ));

        let mut control = AttributeConfiguration::recommended();
        control.activation_groups.push(AttributeActivationGroup {
            id: "control".into(),
            label: "Control".into(),
            members: vec![AttributeKey("control".into())],
        });
        assert_eq!(
            control.validate(),
            Err(AttributeConfigurationError::IneligibleActivationMember(
                "control".into()
            ))
        );

        let mut missing = AttributeConfiguration::recommended();
        missing
            .activation_groups
            .retain(|group| !group.members.contains(&AttributeKey("intensity".into())));
        assert_eq!(
            missing.validate(),
            Err(AttributeConfigurationError::MissingActivationGroup(
                "intensity".into()
            ))
        );
    }

    #[test]
    fn non_recordable_custom_controls_are_placed_but_never_activated() {
        let mut configuration = AttributeConfiguration::recommended();
        add_custom(
            &mut configuration,
            custom_descriptor("vendor.reset", AttributeValueType::Control, false),
            EncoderPlacement::new(EncoderGroup::Control, 2, 2),
        );
        configuration.validate().unwrap();
        assert!(
            configuration
                .activation_group_for(&AttributeKey("vendor.reset".into()))
                .is_none()
        );

        configuration
            .activation_groups
            .push(AttributeActivationGroup {
                id: "vendor.reset".into(),
                label: "Reset".into(),
                members: vec![AttributeKey("vendor.reset".into())],
            });
        assert_eq!(
            configuration.validate(),
            Err(AttributeConfigurationError::IneligibleActivationMember(
                "vendor.reset".into()
            ))
        );
    }
}
