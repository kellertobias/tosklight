use super::{ControlAction, FixtureChannel, GeometryGraph, HeadColorSystem};
use crate::{DirectControlProtocol, SignalLossPolicy};
use light_core::FixtureId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const FIXTURE_PROFILE_SCHEMA_VERSION: u16 = 3;

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

/// One of the five stable orthographic fixture-package drawings.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileProjectionView {
    Top,
    Left,
    Right,
    Front,
    Back,
}

impl ProfileProjectionView {
    pub const ALL: [Self; 5] = [Self::Top, Self::Left, Self::Right, Self::Front, Self::Back];

    pub fn wire(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Left => "left",
            Self::Right => "right",
            Self::Front => "front",
            Self::Back => "back",
        }
    }

    pub fn orientation(self) -> ProfileProjectionOrientation {
        match self {
            Self::Top => ProfileProjectionOrientation::XRightZDown,
            Self::Left => ProfileProjectionOrientation::ZRightYUp,
            Self::Right => ProfileProjectionOrientation::ZLeftYUp,
            Self::Front => ProfileProjectionOrientation::XRightYUp,
            Self::Back => ProfileProjectionOrientation::XLeftYUp,
        }
    }
}

/// The physical axes represented by page right and page up.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileProjectionOrientation {
    XRightZDown,
    ZRightYUp,
    ZLeftYUp,
    XRightYUp,
    XLeftYUp,
}

/// The deterministic mechanical pose used while a drawing was generated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileProjectionPose {
    AuthoredHome,
    MovingDown,
    MovingForward,
}

/// One package-owned SVG projection and the physical coordinate contract around it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileProjectionAsset {
    pub view: ProfileProjectionView,
    pub artwork_asset: String,
    pub view_box_millimetres: [f32; 4],
    pub physical_width_millimetres: f32,
    pub physical_height_millimetres: f32,
    pub origin_millimetres: [f32; 2],
    pub orientation: ProfileProjectionOrientation,
    pub pose: ProfileProjectionPose,
}

/// Revision-owned projections generated from one exact source-model and generator version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileProjectionSet {
    pub source_model_sha256: String,
    pub generator: String,
    pub generator_version: String,
    pub pose_contract_version: u16,
    pub views: Vec<ProfileProjectionAsset>,
}

#[derive(Clone, Debug, Serialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_assets: Option<ProfileProjectionSet>,
    #[serde(default)]
    pub physical: ProfilePhysicalProperties,
    #[serde(default)]
    pub optics: ProfileOptics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laser: Option<ProfileLaser>,
    /// The fixture's gobo wheel, slot by slot, when the package carries one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gobos: Vec<ProfileGobo>,
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

#[derive(Deserialize)]
struct FixtureProfileCanonical {
    schema_version: u16,
    id: FixtureId,
    revision: u32,
    manufacturer: String,
    name: String,
    short_name: String,
    fixture_type: String,
    #[serde(default)]
    patch_policy: PatchPolicy,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    photograph_asset: Option<String>,
    #[serde(default)]
    stage_icon_asset: Option<String>,
    #[serde(default)]
    model_asset: Option<String>,
    #[serde(default)]
    model_units: ModelUnits,
    #[serde(default)]
    projection_assets: Option<ProfileProjectionSet>,
    #[serde(default)]
    physical: ProfilePhysicalProperties,
    #[serde(default)]
    optics: ProfileOptics,
    #[serde(default)]
    laser: Option<ProfileLaser>,
    #[serde(default)]
    gobos: Vec<ProfileGobo>,
    modes: Vec<FixtureMode>,
    #[serde(default)]
    hazardous: bool,
    #[serde(default)]
    direct_control_protocols: Vec<DirectControlProtocol>,
    #[serde(default)]
    signal_loss_policy: SignalLossPolicy,
    #[serde(default)]
    reserved_source: Option<String>,
}

impl<'de> Deserialize<'de> for FixtureProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut canonical = FixtureProfileCanonical::deserialize(deserializer)?;
        if matches!(canonical.schema_version, 2 | FIXTURE_PROFILE_SCHEMA_VERSION) {
            for channel in canonical
                .modes
                .iter_mut()
                .flat_map(|mode| &mut mode.channels)
            {
                let legacy = channel.attribute.clone();
                let migration = if canonical.schema_version == 2 {
                    super::legacy_canonical_mapping(&legacy)
                } else {
                    super::canonical_attribute_mapping(&legacy)
                };
                let Some((attribute, transform)) = migration else {
                    continue;
                };
                channel.attribute = attribute.clone();
                channel.canonical_transform = transform;
                for function in &mut channel.functions {
                    if function.attribute == legacy {
                        function.attribute = attribute.clone();
                    }
                }
            }
        }
        Ok(Self {
            schema_version: if canonical.schema_version == 2 {
                FIXTURE_PROFILE_SCHEMA_VERSION
            } else {
                canonical.schema_version
            },
            id: canonical.id,
            revision: canonical.revision,
            manufacturer: canonical.manufacturer,
            name: canonical.name,
            short_name: canonical.short_name,
            fixture_type: canonical.fixture_type,
            patch_policy: canonical.patch_policy,
            notes: canonical.notes,
            photograph_asset: canonical.photograph_asset,
            stage_icon_asset: canonical.stage_icon_asset,
            model_asset: canonical.model_asset,
            model_units: canonical.model_units,
            projection_assets: canonical.projection_assets,
            physical: canonical.physical,
            optics: canonical.optics,
            laser: canonical.laser,
            gobos: canonical.gobos,
            modes: canonical.modes,
            hazardous: canonical.hazardous,
            direct_control_protocols: canonical.direct_control_protocols,
            signal_loss_policy: canonical.signal_loss_policy,
            reserved_source: canonical.reserved_source,
        })
    }
}

/// How this fixture's light behaves, as against how it is built.
///
/// Two lanterns at the same angle and the same level do not look alike: a profile lays down a flat
/// disc with a rim you could cut paper on, a PAR is hot in the middle inside a soft halo, a flood
/// has no rim at all. These are the numbers that carry that difference. Every one of them is
/// optional, and what a profile leaves out is derived from its declared `fixture_type`, so a
/// library that has never been told any of this still renders sensibly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileOptics {
    /// Relative output, `1.0` being an ordinary fixture of its type: a 400 W engine against a
    /// 100 W one, before anyone touches a dimmer. Absent means "read it from the declared
    /// luminous output, or take the type's own figure".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<f32>,
    /// How hard the rim of the field is, `0.0` to `1.0`. A profile cuts, a Fresnel blends, a wash
    /// has no edge to speak of. A focus or frost channel softens whatever is declared here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpness: Option<f32>,
    /// How evenly the field is filled, `0.0` to `1.0`: `1.0` flat to the rim, `0.0` a bright
    /// centre that falls away quickly. Separate from the rim — a good LED wash has no edge and is
    /// still even across the middle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uniformity: Option<f32>,
    /// The lit surface light leaves through. It belongs to the fixture, not to one patched
    /// instance: every lantern of this type has the same lens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_source: Option<ProfileLightSource>,
}

/// What a laser projector's scanner can do, and the script that decides what it draws.
///
/// A laser is the one fixture class whose output cannot be described by an angle and a colour. Two
/// projectors given identical DMX draw completely different pictures, because almost everything an
/// operator sees is decided inside the fixture's own pattern engine — the DMX only selects and
/// modulates it. So the profile carries the engine as source text rather than trying to enumerate
/// its results: a script the visualizer runs each frame to get the actual path the beam takes.
///
/// Everything here is optional and a missing figure is derived from the declared `fixture_type`,
/// exactly as [`ProfileOptics`] works. A laser package with no measured scanner data still
/// projects; it projects a typical show laser.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileLaser {
    /// The scan engine: a JavaScript module exporting `scan`. Held as a relative `assets/*.js`
    /// path inside a package and as a self-contained data URL at runtime, which is what lets the
    /// script reach a visualizer through a patched show's profile snapshot with no second channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_script_asset: Option<String>,
    /// Full optical scan angle across the X axis in degrees — the whole cone the scanner can
    /// reach, not the half-angle. A script's `x` of `-1..=1` spans exactly this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_angle_degrees: Option<f32>,
    /// Full optical scan angle across the Y axis. Absent means the scanner is square and the X
    /// figure serves both, which is true of most galvanometer pairs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_angle_y_degrees: Option<f32>,
    /// Scanner speed in points per second, the figure a manufacturer quotes as "30 kpps". This is
    /// what decides how many complete scans land inside one rendered frame, and therefore whether
    /// a pattern reads as solid or as a travelling dot. A script may override it per frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points_per_second: Option<f32>,
    /// Beam divergence in milliradians: how much the beam has spread by the time it lands. A show
    /// laser is around `1.0`, which is a millimetre per metre of throw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divergence_milliradians: Option<f32>,
    /// Beam diameter at the output window in millimetres, before divergence opens it up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aperture_millimetres: Option<f32>,
    /// Total optical output in milliwatts, all colours at full. Brightness scales from this, which
    /// is why a 500 mW projector and a 5 W one do not look alike at the same DMX value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optical_power_milliwatts: Option<f32>,
}

/// One slot on the fixture's gobo wheel.
///
/// A gobo channel says which slot is in the beam; it cannot say what is etched on the glass. The
/// wheel is therefore declared here, slot by slot, so a profile projects its own patterns rather
/// than the visualizer's stand-ins — and so the wheel is divided into the number of slots the
/// fixture actually has instead of a guess.
///
/// A profile that declares no wheel keeps the old behaviour exactly: the drawn patterns, evenly
/// divided. A wheel that declares slots but no artwork still gets the right number of them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileGobo {
    /// Which slot this is, counting the open slot as zero. Slots need not be contiguous and the
    /// open slot need not be declared; the wheel is as long as its highest slot.
    pub slot: u32,
    /// What the manual calls this gobo, for the operator surfaces that name a slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The artwork, as a relative `assets/*.png` path inside a package and a self-contained data
    /// URL at runtime — the same journey the model and the scan script make, and for the same
    /// reason: it has to reach a visualizer through a patched show's profile snapshot.
    ///
    /// Light passes where the image is white. Colour is ignored: glass is a mask, and a gobo takes
    /// the colour of whatever the fixture is putting through it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_asset: Option<String>,
}

/// The shape and size of the emitting surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileLightSource {
    #[serde(default)]
    pub form: LightSourceForm,
    pub width_millimetres: f32,
    pub height_millimetres: f32,
}

/// The outline of the emitting surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightSourceForm {
    #[default]
    Round,
    /// Wider than it is tall, or the reverse: a PAR's lens, a linear engine.
    Oval,
    /// A panel: cyc floods, blinders, LED bricks.
    Rectangular,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfilePhysicalProperties {
    #[serde(default)]
    pub width_millimetres: Option<f32>,
    #[serde(default)]
    pub height_millimetres: Option<f32>,
    #[serde(default)]
    pub depth_millimetres: Option<f32>,
    #[serde(default)]
    pub weight_kilograms: Option<f32>,
    #[serde(default)]
    pub power_watts: Option<f32>,
    #[serde(default)]
    pub connectors: String,
    #[serde(default)]
    pub light_source: String,
    #[serde(default)]
    pub color_temperature_kelvin: Option<f32>,
    #[serde(default)]
    pub color_rendering_index: Option<f32>,
    #[serde(default)]
    pub luminous_output_lumens: Option<f32>,
    #[serde(default)]
    pub lens: String,
    #[serde(default)]
    pub beam_angle_degrees: Option<f32>,
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
