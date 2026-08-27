use crate::FixtureDefinition;
use light_core::{AttributeKey, AttributeValue, DmxAddress, FixtureId, Universe};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use uuid::Uuid;

const MAX_APPEARANCE_ID_BYTES: usize = 128;
const MAX_APPEARANCE_LABEL_BYTES: usize = 256;
const MAX_APPEARANCE_NOTE_BYTES: usize = 1_024;

/// Portable, installed appearance of one physical fixture instance.
///
/// Mechanical bracket and shaper-module rotation remain in their established patch fields. This
/// value carries the additional lamp, filter, and four static shaper-element settings that travel
/// with the physical instance rather than its immutable fixture profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstalledFixtureAppearance {
    #[serde(default)]
    pub light_source: InstalledLightSource,
    /// An explicit installed colour temperature. `None` inherits the selected profile revision.
    #[serde(default)]
    pub color_temperature_kelvin: Option<u32>,
    /// Actual output of this installed source in lumens. `None` inherits the profile revision.
    #[serde(default)]
    pub luminous_output_lumens: Option<f32>,
    #[serde(default)]
    pub gel: GelAssignment,
    /// Installed static shutter or barn-door element angles, in element order one through four.
    #[serde(default)]
    pub shaper_angles_degrees: [f32; 4],
}

impl Default for InstalledFixtureAppearance {
    fn default() -> Self {
        Self {
            light_source: InstalledLightSource::ProfileDefault,
            color_temperature_kelvin: None,
            luminous_output_lumens: None,
            gel: GelAssignment::OpenWhite,
            shaper_angles_degrees: [0.0; 4],
        }
    }
}

impl InstalledFixtureAppearance {
    pub fn validate(&self) -> Result<(), String> {
        self.light_source.validate()?;
        if self
            .color_temperature_kelvin
            .is_some_and(|kelvin| !(1_000..=25_000).contains(&kelvin))
        {
            return Err("installed color temperature must be within 1000-25000 K".into());
        }
        if self
            .luminous_output_lumens
            .is_some_and(|lumens| !lumens.is_finite() || lumens <= 0.0)
        {
            return Err("installed luminous output must be a positive finite lumen value".into());
        }
        self.gel.validate()?;
        if self
            .shaper_angles_degrees
            .iter()
            .any(|angle| !angle.is_finite() || !(-180.0..180.0).contains(angle))
        {
            return Err("installed shaper angles must be finite degrees within [-180, 180)".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstalledLightSource {
    #[default]
    ProfileDefault,
    Tungsten,
    Halogen,
    Discharge,
    Led,
    Fluorescent,
    Arc,
    Other {
        label: String,
    },
}

impl InstalledLightSource {
    fn validate(&self) -> Result<(), String> {
        if let Self::Other { label } = self {
            validate_trimmed(
                label,
                "installed light-source label",
                MAX_APPEARANCE_LABEL_BYTES,
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GelAssignment {
    #[default]
    OpenWhite,
    BuiltIn {
        catalog_id: String,
        entry_id: String,
        embedded_fallback: GelDefinitionSnapshot,
    },
    Custom {
        name: String,
        color_srgb: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

impl GelAssignment {
    fn validate(&self) -> Result<(), String> {
        match self {
            Self::OpenWhite => Ok(()),
            Self::BuiltIn {
                catalog_id,
                entry_id,
                embedded_fallback,
            } => {
                validate_trimmed(catalog_id, "gel catalog identity", MAX_APPEARANCE_ID_BYTES)?;
                validate_trimmed(entry_id, "gel entry identity", MAX_APPEARANCE_ID_BYTES)?;
                embedded_fallback.validate()
            }
            Self::Custom {
                name,
                color_srgb,
                note,
            } => {
                validate_trimmed(name, "custom gel name", MAX_APPEARANCE_LABEL_BYTES)?;
                validate_srgb(color_srgb, "custom gel color")?;
                if let Some(note) = note {
                    validate_trimmed(note, "custom gel note", MAX_APPEARANCE_NOTE_BYTES)?;
                }
                Ok(())
            }
        }
    }
}

/// Portable fallback for an assigned catalog entry when its source catalog is unavailable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GelDefinitionSnapshot {
    pub number: String,
    pub name: String,
    pub display_srgb: String,
    pub visualizer_srgb: String,
}

impl GelDefinitionSnapshot {
    fn validate(&self) -> Result<(), String> {
        validate_trimmed(&self.number, "gel catalog number", MAX_APPEARANCE_ID_BYTES)?;
        validate_trimmed(&self.name, "gel display name", MAX_APPEARANCE_LABEL_BYTES)?;
        validate_srgb(&self.display_srgb, "gel display color")?;
        validate_srgb(&self.visualizer_srgb, "gel visualizer color")
    }
}

fn validate_trimmed(value: &str, label: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() || value.trim() != value || value.len() > max_bytes {
        return Err(format!(
            "{label} must be trimmed and contain 1-{max_bytes} bytes"
        ));
    }
    Ok(())
}

fn validate_srgb(value: &str, label: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 7
        || bytes[0] != b'#'
        || bytes[1..]
            .iter()
            .any(|byte| !byte.is_ascii_digit() && !(b'A'..=b'F').contains(byte))
    {
        return Err(format!("{label} must be canonical #RRGGBB sRGB"));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchedFixture {
    pub fixture_id: FixtureId,
    /// Operator-facing fixture number. This is distinct from the stable internal UUID.
    #[serde(default)]
    pub fixture_number: Option<u32>,
    /// Operator-facing number in the reserved visual-only `0.x` namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtual_fixture_number: Option<u32>,
    /// Show-local operator name. Definition names remain immutable library metadata.
    #[serde(default)]
    pub name: String,
    pub definition: FixtureDefinition,
    #[serde(default)]
    pub universe: Option<Universe>,
    /// User-facing DMX address, always 1 through 512.
    #[serde(default)]
    pub address: Option<DmxAddress>,
    /// Schema-v2 independently patchable split assignments. Legacy universe/address remain the
    /// canonical split-1 representation and are migrated into this shape on the next save.
    #[serde(default)]
    pub split_patches: Vec<SplitPatch>,
    #[serde(default = "default_patch_layer")]
    pub layer_id: String,
    /// Optional direct-control endpoint attached to the physical parent fixture.
    /// Logical heads inherit this endpoint and cannot override it.
    #[serde(default)]
    pub direct_control: Option<DirectControlEndpoint>,
    /// Portable logical bindings for an application-owned Internal fixture. Concrete local paths
    /// and device identities remain desk-local and are never stored here.
    #[serde(default, skip_serializing_if = "InternalFixtureBindings::is_empty")]
    pub internal_bindings: InternalFixtureBindings,
    #[serde(default)]
    pub location: FixtureLocation,
    #[serde(default)]
    pub rotation: FixtureVector,
    /// The 3D Point this fixture is slaved to, if any.
    ///
    /// The fixture keeps its own patched location and rotation. Those describe where it sits when
    /// the point rests at its own origin; moving or rotating the point carries the fixture with
    /// it. An absent master leaves the fixture placed against the stage, exactly as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_master: Option<Uuid>,
    #[serde(default)]
    pub logical_heads: Vec<PatchedHead>,
    /// Additional physical instances controlled and selected as this fixture.
    /// An instance without a universe/address exists in the visualizer only.
    #[serde(default)]
    pub multipatch: Vec<MultiPatchInstance>,
    /// Eligible intensity channels participate in Group Master and Group flash scaling.
    #[serde(default = "default_true")]
    pub group_masters_enabled: bool,
    /// Eligible intensity channels participate in Grand Master scaling.
    #[serde(default = "default_true")]
    pub grand_master_enabled: bool,
    /// Reverse the normalized Pan request for this physical fixture.
    #[serde(default)]
    pub invert_pan: bool,
    /// Reverse the normalized Tilt request for this physical fixture.
    #[serde(default)]
    pub invert_tilt: bool,
    /// Degrees the mounting bracket is set to: how far the fixture is angled in the clamp or yoke
    /// it hangs from, positive nose-down. Nothing on the desk can drive it, so the show records
    /// what the rig was actually set to and the picture follows it.
    #[serde(default)]
    pub bracket_angle: f32,
    /// Degrees the fitted shaper or barn-door module is turned to, or `None` when none is fitted.
    ///
    /// A framing module the desk can rotate over DMX starts from this angle; a barn door, which
    /// nothing turns but a hand, only ever has this one.
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    /// Installed source, CCT, gel, and static shaper-element settings for this root instance.
    #[serde(default)]
    pub installed_appearance: InstalledFixtureAppearance,
    /// Preposition Position-family attributes for the next lit Cue while dark.
    #[serde(default = "default_true")]
    pub move_in_black_enabled: bool,
    /// Safety delay measured from the resolved-dark boundary.
    #[serde(default)]
    pub move_in_black_delay_millis: u64,
    /// Optional per-instance raw Highlight overrides keyed by stable channel ID.
    #[serde(default)]
    pub highlight_overrides: BTreeMap<Uuid, u32>,
    /// Portable per-fixture output holds. Freeze is show intent, not Programmer ownership: the
    /// retained values continue to win while ordinary control state evolves underneath them.
    #[serde(default, skip_serializing_if = "FixtureFreezeState::is_empty")]
    pub freeze: FixtureFreezeState,
}

/// Operator-facing attribute families supported by partial fixture Freeze.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreezeFamily {
    Intensity,
    Color,
    Position,
    Beam,
}

impl FreezeFamily {
    pub fn accepts(self, attribute: &AttributeKey) -> bool {
        use light_core::AttributeClass;

        let class = light_core::attribute_descriptor(attribute).family;
        match self {
            Self::Intensity => attribute.is_intensity() || class == AttributeClass::Intensity,
            Self::Color => {
                class == AttributeClass::Color
                    || *attribute.0 == *"color"
                    || attribute.0.starts_with("color.")
                    || attribute.0.contains(".color.")
            }
            Self::Position => attribute.is_position() || class == AttributeClass::Position,
            Self::Beam => {
                matches!(
                    class,
                    AttributeClass::Beam | AttributeClass::Shapers | AttributeClass::Focus
                )
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct FrozenFixtureTarget {
    /// Full Freeze bypasses Group Master, Grand Master, Blackout, Highlight, and later control
    /// changes. Setting it deliberately clears partial-family metadata.
    #[serde(default)]
    pub full: bool,
    #[serde(default)]
    pub families: Vec<FreezeFamily>,
    #[serde(default)]
    pub values: HashMap<AttributeKey, AttributeValue>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct FixtureFreezeState {
    /// Root fixtures and logical heads retain independent Freeze state by stable fixture identity.
    #[serde(default)]
    pub targets: HashMap<FixtureId, FrozenFixtureTarget>,
}

impl FixtureFreezeState {
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct InternalFixtureBindings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl InternalFixtureBindings {
    pub fn is_empty(&self) -> bool {
        self.library.is_none() && self.output.is_none()
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiPatchInstance {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub universe: Option<Universe>,
    #[serde(default)]
    pub address: Option<DmxAddress>,
    #[serde(default)]
    pub split_patches: Vec<SplitPatch>,
    #[serde(default)]
    pub location: FixtureLocation,
    #[serde(default)]
    pub rotation: FixtureVector,
    /// Reverse the normalized Pan request for this physical instance.
    #[serde(default)]
    pub invert_pan: bool,
    /// Reverse the normalized Tilt request for this physical instance.
    #[serde(default)]
    pub invert_tilt: bool,
    /// Degrees the mounting bracket of this physical instance is set to.
    #[serde(default)]
    pub bracket_angle: f32,
    /// Degrees the fitted shaper or barn-door module of this instance is turned to, or `None`
    /// when none is fitted.
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    /// Installed source, CCT, gel, and static shaper-element settings for this physical copy.
    #[serde(default)]
    pub installed_appearance: InstalledFixtureAppearance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SplitPatch {
    pub split: u16,
    pub universe: Option<Universe>,
    pub address: Option<DmxAddress>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FixtureVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct FixtureLocation {
    /// Integer millimetres avoid accumulating floating-point positioning error.
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl<'de> Deserialize<'de> for FixtureLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct StoredLocation {
            #[serde(deserialize_with = "deserialize_location_coordinate")]
            x: i32,
            #[serde(deserialize_with = "deserialize_location_coordinate")]
            y: i32,
            #[serde(deserialize_with = "deserialize_location_coordinate")]
            z: i32,
        }
        let stored = StoredLocation::deserialize(deserializer)?;
        Ok(Self {
            x: stored.x,
            y: stored.y,
            z: stored.z,
        })
    }
}

fn deserialize_location_coordinate<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct CoordinateVisitor;
    impl<'de> serde::de::Visitor<'de> for CoordinateVisitor {
        type Value = i32;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(
                "an integer millimetre coordinate or a legacy floating-point metre coordinate",
            )
        }
        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<i32, E> {
            i32::try_from(value).map_err(E::custom)
        }
        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<i32, E> {
            i32::try_from(value).map_err(E::custom)
        }
        fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<i32, E> {
            let millimetres = value * 1_000.0;
            if !millimetres.is_finite()
                || millimetres < f64::from(i32::MIN)
                || millimetres > f64::from(i32::MAX)
            {
                return Err(E::custom(
                    "legacy fixture location is outside the supported range",
                ));
            }
            Ok(millimetres.round() as i32)
        }
    }
    deserializer.deserialize_any(CoordinateVisitor)
}

pub(crate) fn default_patch_layer() -> String {
    "default".into()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectControlEndpoint {
    pub protocol: DirectControlProtocol,
    pub ip_address: IpAddr,
    #[serde(default = "default_citp_port")]
    pub port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectControlProtocol {
    Citp,
}

const fn default_citp_port() -> u16 {
    4811
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchedHead {
    /// Stable identity of the selected immutable profile head.
    ///
    /// Legacy patch records do not have this value. The next profile-aware patch mutation fills it
    /// after matching that legacy head by `head_index` once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_head_id: Option<Uuid>,
    pub head_index: u16,
    pub fixture_id: FixtureId,
}
