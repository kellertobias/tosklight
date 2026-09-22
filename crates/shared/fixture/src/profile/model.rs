use super::{
    ControlAction, EmitterHeadBinding, FixtureChannel, GeometryGraph, HeadColorSystem,
    MotionAttributeBinding, Vector3,
};
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
    Internal,
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
    /// The fixture's parts, axes and emitters.
    ///
    /// Geometry belongs to the lantern rather than to one of its personalities: a moving head has
    /// the same yoke whichever mode it is patched in. A mode says only which of its heads owns
    /// which emitter, in [`FixtureMode::emitter_heads`].
    ///
    /// Empty on a profile whose modes still carry their own geometry — see that field.
    #[serde(default)]
    pub geometry: GeometryGraph,
    /// The generic body this fixture is drawn as, named from `body_catalogue::BODY_CATALOGUE`.
    ///
    /// `None` keeps the guess made from the declared type and the mode's channels, which is how
    /// every profile behaved before a body could be chosen. A packaged `model_asset` wins over
    /// both: a fixture that ships its own geometry is drawn with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_model: Option<String>,
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
    /// Portable visualizer contract for a visual-only crowd-area fixture.
    ///
    /// The selected fixture mode chooses one entry by stable mode identity. Keeping posture and
    /// density in the transferable package avoids a renderer-owned catalogue of special fixture
    /// names and leaves room for a later animated presentation without changing patch identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crowd: Option<ProfileCrowd>,
    /// A package-owned particle-effect mapping for flame, spark, and future emitter families.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<ProfileEffect>,
    /// Package-owned DMX mapping and physical body contract for scenic elements released on cue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics: Option<ProfilePhysics>,
    /// Present on a Venue or Rigging object whose geometry is generated at the size it is
    /// placed, instead of being drawn from a model made for one size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenery: Option<ProfileScenery>,
    /// How the fixture is hung: the clip it is held by, and where a pipe sits in it.
    ///
    /// Absent on a profile written before clips were declared. Such a fixture is still hung, from
    /// a clip guessed at the top of its box, so old shows keep rigging as they always did; a
    /// declared clip replaces the guess with the real hardware.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounting: Option<ProfileMounting>,
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

/// The physical block as it is read, including the three optical facts it used to carry.
///
/// Colour temperature, luminous output, and beam angle describe the light rather than the lantern,
/// so they now live in [`ProfileOptics`]. Every profile written before that — including the one
/// embedded in an already-patched show — still has them here, and is lifted on read.
#[derive(Default, Deserialize)]
struct LegacyPhysicalProperties {
    #[serde(default)]
    width_millimetres: Option<f32>,
    #[serde(default)]
    height_millimetres: Option<f32>,
    #[serde(default)]
    depth_millimetres: Option<f32>,
    #[serde(default)]
    weight_kilograms: Option<f32>,
    #[serde(default)]
    power_watts: Option<f32>,
    #[serde(default)]
    connectors: String,
    #[serde(default)]
    light_source: String,
    #[serde(default)]
    color_rendering_index: Option<f32>,
    #[serde(default)]
    lens: String,
    #[serde(default)]
    color_temperature_kelvin: Option<f32>,
    #[serde(default)]
    luminous_output_lumens: Option<f32>,
    #[serde(default)]
    beam_angle_degrees: Option<f32>,
}

impl LegacyPhysicalProperties {
    /// Splits the read block into how the fixture is built and what its light is like. An optics
    /// value already written wins: it is the newer statement of the same fact.
    fn split(self, optics: &mut ProfileOptics) -> ProfilePhysicalProperties {
        optics.color_temperature_kelvin = optics
            .color_temperature_kelvin
            .or(self.color_temperature_kelvin);
        optics.luminous_output_lumens = optics
            .luminous_output_lumens
            .or(self.luminous_output_lumens);
        optics.beam_angle_degrees = optics.beam_angle_degrees.or(self.beam_angle_degrees);
        ProfilePhysicalProperties {
            width_millimetres: self.width_millimetres,
            height_millimetres: self.height_millimetres,
            depth_millimetres: self.depth_millimetres,
            weight_kilograms: self.weight_kilograms,
            power_watts: self.power_watts,
            connectors: self.connectors,
            light_source: self.light_source,
            color_rendering_index: self.color_rendering_index,
            lens: self.lens,
        }
    }
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
    body_model: Option<String>,
    #[serde(default)]
    geometry: GeometryGraph,
    #[serde(default)]
    model_units: ModelUnits,
    #[serde(default)]
    projection_assets: Option<ProfileProjectionSet>,
    #[serde(default)]
    physical: LegacyPhysicalProperties,
    #[serde(default)]
    optics: ProfileOptics,
    #[serde(default)]
    laser: Option<ProfileLaser>,
    #[serde(default)]
    crowd: Option<ProfileCrowd>,
    effect: Option<ProfileEffect>,
    #[serde(default)]
    physics: Option<ProfilePhysics>,
    #[serde(default)]
    scenery: Option<ProfileScenery>,
    #[serde(default)]
    mounting: Option<ProfileMounting>,
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

/// Lift geometry from the modes to the fixture, where a profile's modes agree about it.
///
/// A lantern has one set of parts, axes and emitters; a personality only decides which of its
/// heads drives which of them. Profiles written before that carry a whole graph per mode, and
/// almost all of them carry the *same* graph per mode, differing only in the head each emitter
/// names — which is exactly the part that belongs to the mode.
///
/// Where the graphs genuinely differ the profile is left as it was. Those are real differences —
/// a blinder family whose modes are different physical fixtures, a curtain whose modes are its
/// widths — and they are resolved by reworking the fixture, not by a reader picking one mode's
/// geometry and discarding the rest.
/// What a node or emitter is, with nothing that only says which mode wrote it.
///
/// Identifiers were regenerated per mode by the authoring that produced these profiles, so two
/// modes describing the same part of the same lantern say so in every field but the UUID. Parents
/// are compared by their position in the list, which is the same information without the identity.
fn geometry_content(graph: &GeometryGraph) -> (Vec<String>, Vec<String>) {
    let position =
        |id: Option<Uuid>| id.and_then(|id| graph.nodes.iter().position(|node| node.id == id));
    let nodes = graph
        .nodes
        .iter()
        .map(|node| {
            let mut value = serde_json::to_value(node).unwrap_or_default();
            if let Some(object) = value.as_object_mut() {
                object.remove("id");
                // Which attribute turns an axis is the mode's answer, like an emitter's head, so
                // modes that name different attributes for the same axis still share the lantern.
                if let Some(motion) = object
                    .get_mut("motion")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    motion.remove("attribute");
                }
                object.insert(
                    "parent_id".into(),
                    serde_json::json!(position(node.parent_id)),
                );
            }
            value.to_string()
        })
        .collect();
    let emitters = graph
        .emitters
        .iter()
        .map(|emitter| {
            let mut value = serde_json::to_value(emitter).unwrap_or_default();
            if let Some(object) = value.as_object_mut() {
                object.remove("id");
                object.remove("head_id");
                object.insert(
                    "node_id".into(),
                    serde_json::json!(position(Some(emitter.node_id))),
                );
            }
            value.to_string()
        })
        .collect();
    (nodes, emitters)
}

/// Lift geometry from the modes to the fixture, where a profile's modes agree about it.
///
/// A lantern has one set of parts, axes and emitters; a personality only decides which of its
/// heads drives which of them. Profiles written before that carry a whole graph per mode, and
/// almost all of them describe the same lantern in each — some identically, and some with a mode
/// that adds parts the others leave out, which is a personality driving more of the same fixture
/// rather than a different fixture. The fullest graph is the lantern, and the rest have to be
/// prefixes of it to qualify.
///
/// Where the graphs genuinely disagree the profile is left as it was. Those are real differences —
/// a curtain whose modes are its widths — and they are resolved by reworking the fixture, not by a
/// reader picking one mode's geometry and discarding the rest.
fn lift_geometry_to_the_fixture(profile: &mut FixtureProfile) {
    if !profile.geometry.nodes.is_empty() || profile.modes.is_empty() {
        return;
    }
    let contents = profile
        .modes
        .iter()
        .map(|mode| geometry_content(&mode.geometry))
        .collect::<Vec<_>>();
    let Some(fullest) = (0..contents.len()).max_by_key(|index| {
        (
            contents[*index].0.len(),
            contents[*index].1.len(),
            usize::MAX - index,
        )
    }) else {
        return;
    };
    let (nodes, emitters) = &contents[fullest];
    let prefix_of = |part: &[String], whole: &[String]| whole.starts_with(part);
    if contents.iter().any(|(mode_nodes, mode_emitters)| {
        !prefix_of(mode_nodes, nodes) || !prefix_of(mode_emitters, emitters)
    }) {
        return;
    }

    let mut lifted = profile.modes[fullest].geometry.clone();
    let lifted_emitters = lifted
        .emitters
        .iter()
        .map(|emitter| emitter.id)
        .collect::<Vec<_>>();
    let lifted_nodes = lifted.nodes.iter().map(|node| node.id).collect::<Vec<_>>();
    for mode in &mut profile.modes {
        // A mode's emitters are the first n of the fixture's, so position carries the identity
        // across from whichever mode happened to be written with which UUID.
        mode.emitter_heads = mode
            .geometry
            .emitters
            .iter()
            .enumerate()
            .filter_map(|(index, emitter)| {
                Some(EmitterHeadBinding {
                    emitter_id: *lifted_emitters.get(index)?,
                    head_id: emitter.head_id?,
                })
            })
            .collect();
        // The same holds for the axes: a mode's nodes are the first n of the fixture's, and the
        // attribute each of them named stays with the mode that named it.
        for (index, node) in mode.geometry.nodes.iter().enumerate() {
            let (Some(node_id), Some(attribute)) = (
                lifted_nodes.get(index),
                node.motion
                    .as_ref()
                    .and_then(|motion| motion.attribute.clone()),
            ) else {
                continue;
            };
            if !mode
                .motion_attributes
                .iter()
                .any(|binding| binding.node_id == *node_id)
            {
                mode.motion_attributes.push(MotionAttributeBinding {
                    node_id: *node_id,
                    attribute,
                });
            }
        }
        mode.geometry = GeometryGraph::default();
    }
    // The fixture's own emitters name no head and its axes no attribute: those are the mode's
    // answers now.
    for emitter in &mut lifted.emitters {
        emitter.head_id = None;
    }
    for motion in lifted
        .nodes
        .iter_mut()
        .filter_map(|node| node.motion.as_mut())
    {
        motion.attribute = None;
    }
    profile.geometry = lifted;
}

/// Move the attribute an already-lifted fixture graph still names on an axis into the modes.
///
/// Profiles lifted before the attribute became the mode's answer carry it on the fixture's own
/// node, where it spoke for every mode. So every mode that does not already bind that axis is
/// given the attribute the node named, and the node stops naming one. A mode that binds the axis
/// itself keeps its own answer.
fn move_motion_attributes_to_the_modes(profile: &mut FixtureProfile) {
    for node in &mut profile.geometry.nodes {
        let Some(motion) = node.motion.as_mut() else {
            continue;
        };
        let Some(attribute) = motion.attribute.take() else {
            continue;
        };
        for mode in &mut profile.modes {
            if !mode
                .motion_attributes
                .iter()
                .any(|binding| binding.node_id == node.id)
            {
                mode.motion_attributes.push(MotionAttributeBinding {
                    node_id: node.id,
                    attribute: attribute.clone(),
                });
            }
        }
    }
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
        let mut profile = Self {
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
            body_model: canonical.body_model,
            geometry: canonical.geometry,
            model_units: canonical.model_units,
            projection_assets: canonical.projection_assets,
            physical: canonical.physical.split(&mut canonical.optics),
            optics: canonical.optics,
            laser: canonical.laser,
            crowd: canonical.crowd,
            effect: canonical.effect,
            physics: canonical.physics,
            scenery: canonical.scenery,
            mounting: canonical.mounting,
            gobos: canonical.gobos,
            modes: canonical.modes,
            hazardous: canonical.hazardous,
            direct_control_protocols: canonical.direct_control_protocols,
            signal_loss_policy: canonical.signal_loss_policy,
            reserved_source: canonical.reserved_source,
        };
        lift_geometry_to_the_fixture(&mut profile);
        move_motion_attributes_to_the_modes(&mut profile);
        Ok(profile)
    }
}

/// How a fixture is hung: its mounting clip, as a volume a pipe has to reach into.
///
/// Every measurement is in millimetres in the fixture's own axes, from the centre of its body:
/// `x` across, `y` deep, `z` up, the same axes the plan places it in. A lantern is declared by the
/// hardware it really carries — the hook clamp on its yoke, the half-coupler on its bracket —
/// rather than by a slice of its bounding box, because that is what has to meet the pipe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileMounting {
    /// What the fixture is hung by, and whether it can be hung at all.
    #[serde(default)]
    pub hardware: MountingHardware,
    /// The middle of the clip.
    #[serde(default)]
    pub centre_millimetres: Vector3,
    /// Half the clip's reach across, deep and up: how near a pipe must come to be caught.
    #[serde(default)]
    pub half_extent_millimetres: Vector3,
    /// Where the pipe's axis lies once the fixture hangs from the clip.
    #[serde(default)]
    pub pipe_millimetres: Vector3,
    /// The body these measurements were taken against.
    ///
    /// A fixture is drawn at whatever size the plan gives it — its declared physical size, a size
    /// a family of bodies falls back to, or that again scaled by the operator — so the clip is
    /// carried over to the body actually drawn in the same proportion it was authored in. Zero on
    /// any axis means the clip is taken as it stands.
    #[serde(default)]
    pub body_millimetres: Vector3,
}

/// What a fixture is hung by.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MountingHardware {
    /// A hook clamp or half-coupler that closes over a pipe: how a lantern usually hangs.
    #[default]
    Clamp,
    /// A yoke, bracket or baseplate bolted to a surface. It carries the fixture but meets no pipe.
    Yoke,
    /// Nothing to hang the fixture by: a dimmer in a rack, a strip taped to a truss, a floor can.
    None,
}

impl MountingHardware {
    /// Whether the fixture can be caught by a pipe at all.
    pub fn hangs_on_a_pipe(&self) -> bool {
        matches!(self, Self::Clamp)
    }
}

/// Fixture-package declaration for a scalable audience area.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileCrowd {
    /// Suggested authored footprint before the operator scales the placed Venue fixture.
    pub default_width_metres: f32,
    pub default_depth_metres: f32,
    /// Stable selected-mode mappings. Every crowd mode must appear exactly once.
    pub modes: Vec<ProfileCrowdMode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileCrowdMode {
    pub mode_id: Uuid,
    pub posture: CrowdPosture,
    pub density: CrowdDensity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrowdPosture {
    Sitting,
    StandingStill,
    Dancing,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrowdDensity {
    Sparse,
    Medium,
    Dense,
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
    /// Correlated colour temperature of the engine, in kelvin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_temperature_kelvin: Option<f32>,
    /// Total output in lumens, as the manufacturer measures it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminous_output_lumens: Option<f32>,
    /// Nominal beam angle in degrees. A zoom channel's own range overrides this while it moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beam_angle_degrees: Option<f32>,
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

/// The transferable engine that maps one Effect fixture's real DMX programs to emitters.
///
/// The JavaScript itself remains manufacturer-owned and travels in the fixture package. The
/// renderer only understands the bounded, versioned declarative result, so adding a device never
/// adds a fixture-specific program to ToskLight.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileEffect {
    /// An ES module exporting `effect(input)`, stored as `assets/effect.js` in a package and as a
    /// self-contained data URL in a live profile snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_script_asset: Option<String>,
    /// Declarative result contract accepted from this script. Version 1 supports flame and spark
    /// emitters; later versions may add physics-backed debris without changing fixture kind.
    #[serde(default = "default_effect_result_version")]
    pub result_version: u16,
}

/// A portable, renderer-neutral physics body controlled by exact raw fixture slots.
///
/// The script maps DMX to a versioned `release`, `reset`, or `hold` instruction. Integration and
/// latching remain renderer-owned, so a broken or malicious package cannot replace the solver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfilePhysics {
    /// ES module exporting `physics(input)`, packaged as `assets/physics.js`.
    pub control_script_asset: Option<String>,
    #[serde(default = "default_physics_result_version")]
    pub result_version: u16,
    /// Body dimensions in metres. The authored fixture position is the body's hanging centre.
    pub size_metres: [f32; 3],
    #[serde(default)]
    pub scenery_kind: ProfilePhysicsSceneryKind,
    #[serde(default = "default_physics_mass")]
    pub mass_kilograms: f32,
    #[serde(default = "default_physics_gravity")]
    pub gravity_metres_per_second_squared: f32,
    /// Floor plane in world metres. Falling bodies settle with their lower face on this plane.
    #[serde(default)]
    pub floor_y_metres: f32,
    #[serde(default = "default_true")]
    pub scenery_collision: bool,
    #[serde(default)]
    pub self_collision: bool,
}

/// A Venue or Rigging object whose geometry is generated at the size it is placed.
///
/// A curtain's height, a truss's length and a deck's rise are measurements of the venue, not
/// personalities of a fixture. Shipping one profile per size — and a mode per size inside it —
/// described the same object over and over and still only covered the sizes somebody thought of.
/// A profile that declares this instead says what shape it is, and the renderer draws it at
/// whatever size it is patched at: a truss repeats its chords over the length rather than being
/// stretched to it, which is the difference between generating a truss and scaling a picture.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileScenery {
    pub kind: ProfileSceneryKind,
    /// Chords in a truss cross-section: 1 a pipe, 2 a ladder, 3 a triangle, 4 a box. Every other
    /// kind ignores it.
    #[serde(default)]
    pub chords: u8,
    /// The size one is placed at before an operator says otherwise, in metres.
    pub default_size_metres: Vector3,
    /// What an operator may set. A curtain is made to measure in every direction; a truss is the
    /// length of the sticks it is built from and keeps its cross-section.
    #[serde(default)]
    pub adjustable: SceneryAxes,
    /// Bounds for what an operator may set, in metres. A size outside them is clamped.
    pub minimum_size_metres: Vector3,
    pub maximum_size_metres: Vector3,
    /// How a truss is braced: straight zig-zag bays, or the deco pattern. Every other kind ignores
    /// it, and every truss written before the choice existed reads as standard.
    #[serde(default, skip_serializing_if = "TrussPattern::is_standard")]
    pub pattern: TrussPattern,
    /// What a stage element stands on: a scissor lift, or four fixed legs. Every other kind
    /// ignores it, and every riser written before the choice existed reads as a scissor lift,
    /// which is what the shipped stage elements were until fixed feet were generated too.
    #[serde(default, skip_serializing_if = "RiserFeet::is_scissor")]
    pub feet: RiserFeet,
}

/// What a stage element stands on between the floor and its deck.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiserFeet {
    /// Crossed arms over a base frame, as a lift deck is raised.
    #[default]
    Scissor,
    /// Four fixed legs under the corners of the deck, as a staging deck is built.
    Fixed,
}

impl RiserFeet {
    pub fn is_scissor(&self) -> bool {
        *self == Self::Scissor
    }
}

/// The bracing of a truss section.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrussPattern {
    /// Zig-zag diagonals between the chords, bay after bay.
    #[default]
    Standard,
    /// Crossed diagonals in every bay, as decorative truss is built.
    Deco,
}

impl TrussPattern {
    pub fn is_standard(&self) -> bool {
        *self == Self::Standard
    }
}

/// Which of a scenery object's dimensions an operator sets.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SceneryAxes {
    #[serde(default)]
    pub width: bool,
    #[serde(default)]
    pub height: bool,
    #[serde(default)]
    pub depth: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSceneryKind {
    Riser,
    Truss,
    Curtain,
    Railing,
    MirrorBall,
    /// A rigging chain hanging its length, with a hoist or a direct fixing at the top and a direct
    /// fixing or a steelflex loop at the bottom, as the placement chooses.
    Chain,
    /// A plain rectangular block filling its width, height and depth.
    Box,
    /// A cylinder standing upright: its height is its length, and its width and depth are the
    /// diameters across it, so equal ones make it round.
    Cylinder,
    /// A ball filling its width, height and depth, round when all three are equal.
    Sphere,
    #[default]
    Prop,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilePhysicsSceneryKind {
    #[default]
    Curtain,
    Prop,
}

fn default_physics_result_version() -> u16 {
    1
}
fn default_physics_mass() -> f32 {
    1.0
}
fn default_physics_gravity() -> f32 {
    9.806_65
}
fn default_true() -> bool {
    true
}

fn default_effect_result_version() -> u16 {
    1
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
    pub color_rendering_index: Option<f32>,
    #[serde(default)]
    pub lens: String,
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
    /// Which of the fixture's emitters each of this mode's heads owns.
    ///
    /// Empty while the mode still carries its own `geometry`, which is how every profile written
    /// before the lift reads.
    #[serde(default)]
    pub emitter_heads: Vec<EmitterHeadBinding>,
    /// Which attribute drives each of the fixture's motion axes in this mode.
    ///
    /// Empty while the mode still carries its own `geometry`, whose nodes name their attributes.
    #[serde(default)]
    pub motion_attributes: Vec<MotionAttributeBinding>,
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
