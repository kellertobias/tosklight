//! Nominal, estimated physical Position models for modes that author none.
//!
//! The Live Position adapter, the engine's physical projection (Stage/preview) and the Position
//! readouts all evaluate `FixtureMode::position_physical` through `CompiledPositionForward` and
//! `CompiledPositionFitting`. A mode with Pan and Tilt channels but without that model gets no
//! semantic Angles at all. This module derives one, in the transient runtime projection only
//! ([`super::apply_runtime_profile_compatibility`]), so it never changes stored profile bytes,
//! digests or package files:
//!
//! - one absolute Pan channel and one absolute Tilt channel on the same head (16-bit coarse/fine
//!   pairs are one channel with a secondary slot) drive a Pan node and a Tilt node beneath it;
//! - travel comes from the function's declared physical range in degrees where the profile has
//!   one, re-centred so the neutral pose (Angles 0°/0°) is the centre of travel, which is also
//!   where the Stage's legacy proxy puts the yoke. Without declared degrees the travel is the
//!   documented nominal default, Pan [`NOMINAL_PAN_TRAVEL_DEGREES`] and Tilt
//!   [`NOMINAL_TILT_TRAVEL_DEGREES`], centred (the Stage's existing fallback travel);
//! - existing Pan/Tilt motion nodes of the fixture graph are reused (their travel is set to the
//!   derived travel so the Stage proxy and the physical model agree); a graph without them gets
//!   nominal coincident pivots. The lens sits on the Tilt axis, as in the shipped AURO template;
//! - the geometry contract is `estimated` with [`DERIVED_POSITION_SOURCE`] and every mapping is
//!   `unknown` quality, so every readout is labelled estimated/uncalibrated, never measured.
//!
//! Modes it cannot describe honestly get no model and a named [`PositionDerivationExclusion`]:
//! Media model rotation, a laser's scan engine, endless rotation, a single axis, ambiguous or
//! split channels, an authored geometry contract without bindings, or an authored lens that does
//! not follow the Tilt axis. A derived model that does not compile through the existing
//! Position compiler is dropped as well (no parallel solver, no partial model).
use super::{
    AngularMotion, AngularMotionKind, ChannelFunction, ChannelFunctionBehavior, EmitterLayout,
    FixtureChannel, FixtureMode, FixtureProfile, GeometryBracket, GeometryEmitter, GeometryGraph,
    GeometryMotion, GeometryMotionKind, GeometryNode, GeometryPhysicalContract,
    MotionFunctionBinding, OpticalProvenance, PhysicalDataQuality, PhysicalMappingCalibration,
    PhysicalUnit, PositionAxisRole, PositionPhysicalModel, Transform3, Vector3,
};
use light_core::AttributeKey;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Provenance text of every derived Position geometry contract.
pub const DERIVED_POSITION_SOURCE: &str = "Estimated: nominal Position geometry derived at runtime from the profile's Pan/Tilt channels; coincident pivots, lens on the Tilt axis, neutral pose at the centre of travel; zero, direction and pivots unverified; not fixture data";
const DECLARED_MAPPING_SOURCE: &str = "Declared Pan/Tilt travel of the profile, re-centred on the neutral pose; zero and direction unverified";
const NOMINAL_MAPPING_SOURCE: &str =
    "Nominal Pan/Tilt travel: the profile declares no travel in degrees";
/// Nominal travel used when a Pan channel declares no physical range in degrees.
pub const NOMINAL_PAN_TRAVEL_DEGREES: f32 = 540.0;
/// Nominal travel used when a Tilt channel declares no physical range in degrees.
pub const NOMINAL_TILT_TRAVEL_DEGREES: f32 = 270.0;

/// Why a mode with Pan/Tilt-like channels gets no derived Position model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionDerivationExclusion {
    /// `media.model.pan/tilt` rotate a Media server's 3D model, not a lamp's beam.
    MediaModelRotation,
    /// A laser's Pan/Tilt are its scan engine's deflection; the lamp is aimed by its bracket.
    LaserScanEngine,
    /// Endless (continuous) rotation has no absolute angle to fit.
    EndlessRotation,
    /// Only Pan or only Tilt: the Position family is a complete Pan/Tilt pair.
    SingleAxis,
    /// More than one Pan or Tilt channel, or a separate fine channel the model cannot pair.
    AmbiguousChannels,
    /// Pan and Tilt live on different heads.
    SplitHeads,
    /// No continuous absolute function on a Pan or Tilt channel.
    NoAbsoluteFunction,
    /// The fixture graph authors a physical contract; it is never overwritten by an estimate.
    AuthoredGeometryContract,
    /// An authored lens of the Pan/Tilt head does not follow the Tilt axis.
    AuthoredLensOffTiltAxis,
    /// The derived model did not validate or compile through the Position compiler.
    Invalid,
}

/// What the runtime projection does with one mode's Position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionDerivation {
    /// No Pan/Tilt-like channel at all.
    NotApplicable,
    /// The profile authors `position_physical`; it is left untouched.
    Authored,
    /// A nominal model is derived.
    Derived,
    Excluded(PositionDerivationExclusion),
}

/// Give every mode without an authored Position model the derived one, when its Pan/Tilt
/// channels can be described completely. Authored models are never touched.
pub fn apply_derived_position_physical(profile: &mut FixtureProfile) {
    let valid = profile.validate().is_ok();
    for index in 0..profile.modes.len() {
        derive_mode(profile, index, valid);
    }
}

/// Classify one mode exactly as [`apply_derived_position_physical`] would treat it.
pub fn position_derivation(profile: &FixtureProfile, mode_id: Uuid) -> PositionDerivation {
    let Some(index) = profile.modes.iter().position(|m| m.id == mode_id) else {
        return PositionDerivation::NotApplicable;
    };
    if profile.modes[index].position_physical.is_some() {
        return PositionDerivation::Authored;
    }
    let mut trial = profile.clone();
    let valid = trial.validate().is_ok();
    derive_mode(&mut trial, index, valid)
}

/// Name of the lens added to a derived graph whose Pan/Tilt head authors none.
pub const DERIVED_POSITION_LENS_NAME: &str = "Derived Position lens";

/// The graph as the Stage draws it: a derived model's added lens exists only so the Position
/// fitter has an aim; the Stage keeps its own fallback emitter layout and optics for that head.
pub fn without_derived_position_lens(mut graph: GeometryGraph) -> GeometryGraph {
    if is_derived_position_geometry(&graph) {
        graph
            .emitters
            .retain(|e| e.name != DERIVED_POSITION_LENS_NAME);
    }
    graph
}

/// Whether a graph carries a derived (not authored) Position contract.
pub fn is_derived_position_geometry(graph: &GeometryGraph) -> bool {
    graph
        .physical_contract
        .as_ref()
        .is_some_and(|c| c.provenance.source.as_deref() == Some(DERIVED_POSITION_SOURCE))
}

/// `valid`: the profile validated before any derivation. A derived model never turns a valid
/// profile invalid; the whole patch would otherwise be refused for an estimate.
fn derive_mode(profile: &mut FixtureProfile, index: usize, valid: bool) -> PositionDerivation {
    if profile.modes[index].position_physical.is_some() {
        return PositionDerivation::Authored;
    }
    let plan = match plan(profile, &profile.modes[index]) {
        Ok(Some(plan)) => plan,
        Ok(None) => return PositionDerivation::NotApplicable,
        Err(reason) => return PositionDerivation::Excluded(reason),
    };
    let original = profile.modes[index].clone();
    install(&mut profile.modes[index], plan);
    let mode_id = profile.modes[index].id;
    // The compiler validates the graph, the bindings and the native function domains.
    let compiled = matches!(
        super::CompiledPositionFitting::compile(profile, mode_id, Default::default()),
        Ok(Some(_))
    ) && (!valid || profile.validate().is_ok());
    if !compiled {
        profile.modes[index] = original;
        return PositionDerivation::Excluded(PositionDerivationExclusion::Invalid);
    }
    PositionDerivation::Derived
}

fn derived_id(tag: &str, first: Uuid, second: Uuid) -> Uuid {
    let digest = Sha256::digest(
        [
            b"tosklight.derived-position-physical.v1:".as_slice(),
            tag.as_bytes(),
            first.as_bytes(),
            second.as_bytes(),
        ]
        .concat(),
    );
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // RFC 4122 variant, version 8 (vendor-specific): stable and never nil.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Dotted-segment match, so `media.model.pan`, `pan.continuous` and a separate `pan_fine` byte
/// are seen (and then named), while `fixture.pan_tilt_speed` (a speed) is not an angle.
fn mentions(attribute: &str, axis: &str) -> bool {
    attribute.split('.').any(|segment| {
        segment == axis
            || segment
                .strip_prefix(axis)
                .is_some_and(|rest| matches!(rest, "_fine" | "fine" | "_lsb" | "_16"))
    })
}

fn channel_mentions(channel: &FixtureChannel, axis: &str) -> bool {
    mentions(&channel.attribute.0, axis)
        || mentions(&channel.fixture_attribute.0, axis)
        || channel
            .functions
            .iter()
            .any(|f| mentions(&f.attribute.0, axis))
}

fn is_axis(channel: &FixtureChannel, axis: &str) -> bool {
    *channel.attribute.0 == *axis || *channel.fixture_attribute.0 == *axis
}

struct AxisPlan {
    channel: usize,
    /// Existing function to bind, or None to add one spanning the channel.
    function: Option<usize>,
    function_id: Uuid,
    range: (f32, f32),
    declared: bool,
}

struct Plan {
    pan: AxisPlan,
    tilt: AxisPlan,
    graph: GeometryGraph,
    pan_node: Uuid,
    tilt_node: Uuid,
}

fn plan(
    profile: &FixtureProfile,
    mode: &FixtureMode,
) -> Result<Option<Plan>, PositionDerivationExclusion> {
    use PositionDerivationExclusion as X;
    let candidates: Vec<_> = mode
        .channels
        .iter()
        .filter(|c| channel_mentions(c, "pan") || channel_mentions(c, "tilt"))
        .collect();
    if candidates.is_empty() {
        return Ok(None);
    }
    if candidates
        .iter()
        .any(|c| c.attribute.0.starts_with("media.") || c.fixture_attribute.0.starts_with("media."))
    {
        return Err(X::MediaModelRotation);
    }
    if profile.fixture_type.to_ascii_lowercase().contains("laser") {
        return Err(X::LaserScanEngine);
    }
    let endless = |c: &&FixtureChannel| {
        [&c.attribute.0, &c.fixture_attribute.0]
            .iter()
            .any(|a| a.contains("continuous") || a.contains("endless"))
            || c.functions.iter().any(|f| {
                f.angular_motion
                    .is_some_and(|m| m.kind == AngularMotionKind::AngularVelocity)
            })
    };
    if candidates.iter().any(endless) {
        return Err(X::EndlessRotation);
    }
    let pans: Vec<_> = (0..mode.channels.len())
        .filter(|&i| is_axis(&mode.channels[i], "pan"))
        .collect();
    let tilts: Vec<_> = (0..mode.channels.len())
        .filter(|&i| is_axis(&mode.channels[i], "tilt"))
        .collect();
    // Anything else naming Pan/Tilt (a separate fine byte, an alias) would stay unowned.
    if candidates.len() != pans.len() + tilts.len() {
        return Err(X::AmbiguousChannels);
    }
    let (pan, tilt) = match (pans.as_slice(), tilts.as_slice()) {
        ([pan], [tilt]) => (*pan, *tilt),
        ([], _) | (_, []) => return Err(X::SingleAxis),
        _ => return Err(X::AmbiguousChannels),
    };
    let head = mode.channels[pan].head_id;
    if mode.channels[tilt].head_id != head {
        return Err(X::SplitHeads);
    }
    let pan = axis_plan(mode, pan, NOMINAL_PAN_TRAVEL_DEGREES, "pan")?;
    let tilt = axis_plan(mode, tilt, NOMINAL_TILT_TRAVEL_DEGREES, "tilt")?;
    let mut graph = profile.mode_geometry(mode);
    if graph.physical_contract.is_some() {
        return Err(X::AuthoredGeometryContract);
    }
    let (pan_node, tilt_node) = motion_nodes(profile, mode, &mut graph, &pan, &tilt);
    let lens_on_tilt = |emitter: &GeometryEmitter| {
        let mut cursor = Some(emitter.node_id);
        while let Some(id) = cursor {
            if id == tilt_node {
                return true;
            }
            cursor = graph
                .nodes
                .iter()
                .find(|n| n.id == id)
                .and_then(|n| n.parent_id);
        }
        false
    };
    let owned: Vec<_> = graph
        .emitters
        .iter()
        .filter(|e| e.head_id.is_none_or(|h| h == head))
        .collect();
    if owned.iter().any(|e| !lens_on_tilt(e)) {
        return Err(X::AuthoredLensOffTiltAxis);
    }
    if owned.is_empty() {
        let beam = profile
            .optics
            .beam_angle_degrees
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(20.0);
        graph.emitters.push(GeometryEmitter {
            id: derived_id("lens", profile.id.0, mode.id),
            name: DERIVED_POSITION_LENS_NAME.into(),
            node_id: tilt_node,
            head_id: Some(head),
            origin: Vector3::default(),
            orientation_degrees: Vector3::default(),
            beam_angle_degrees: beam,
            field_angle_degrees: beam,
            feather: 0.0,
            focus: 0.0,
            directional: true,
            layout: EmitterLayout::Point,
        });
    }
    graph.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance {
            quality: PhysicalDataQuality::Estimated,
            source: Some(DERIVED_POSITION_SOURCE.into()),
            revision: 1,
        },
        bracket: GeometryBracket::Fixed,
    });
    Ok(Some(Plan {
        pan,
        tilt,
        graph,
        pan_node,
        tilt_node,
    }))
}

/// The channel's absolute function and its centred travel in degrees.
fn axis_plan(
    mode: &FixtureMode,
    channel: usize,
    nominal: f32,
    axis: &str,
) -> Result<AxisPlan, PositionDerivationExclusion> {
    let c = &mode.channels[channel];
    if c.functions.is_empty() {
        let (min, max, declared) =
            travel(c.physical_min, c.physical_max, c.unit.as_deref(), nominal);
        return Ok(AxisPlan {
            channel,
            function: None,
            function_id: derived_id(axis, c.id, mode.id),
            range: (min, max),
            declared,
        });
    }
    // The widest continuous function that is this axis' own (not a speed or reset range).
    let chosen = c
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            matches!(f.behavior, ChannelFunctionBehavior::Continuous { .. })
                && (f.attribute == c.attribute || mentions(&f.attribute.0, axis))
        })
        .max_by_key(|(_, f)| f.dmx_to.saturating_sub(f.dmx_from));
    let Some((index, function)) = chosen else {
        return Err(PositionDerivationExclusion::NoAbsoluteFunction);
    };
    let ChannelFunctionBehavior::Continuous {
        physical_min,
        physical_max,
        unit,
    } = &function.behavior
    else {
        unreachable!()
    };
    let (min, max, declared) = travel(
        Some(*physical_min),
        Some(*physical_max),
        unit.as_deref(),
        nominal,
    );
    Ok(AxisPlan {
        channel,
        function: Some(index),
        function_id: function.id,
        range: (min, max),
        declared,
    })
}

/// Declared degrees re-centred on the neutral pose, or the nominal centred travel.
fn travel(
    min: Option<f32>,
    max: Option<f32>,
    unit: Option<&str>,
    nominal: f32,
) -> (f32, f32, bool) {
    if let (Some(min), Some(max)) = (min, max)
        && PhysicalUnit::parse(unit) == PhysicalUnit::Degrees
        && min.is_finite()
        && max.is_finite()
        && min != max
    {
        if min < 0.0 && max > 0.0 || min > 0.0 && max < 0.0 {
            return (min, max, true);
        }
        let half = (max - min) / 2.0;
        return (-half, half, true);
    }
    (-nominal / 2.0, nominal / 2.0, false)
}

fn motion(attribute: &str, axis: Vector3, range: (f32, f32)) -> GeometryMotion {
    GeometryMotion {
        attribute: Some(AttributeKey(attribute.into())),
        kind: GeometryMotionKind::Rotation,
        axis,
        physical_min: range.0.min(range.1),
        physical_max: range.0.max(range.1),
        max_speed_per_second: None,
        acceleration_per_second_squared: None,
        deceleration_per_second_squared: None,
    }
}

fn node(
    id: Uuid,
    name: &str,
    parent: Option<Uuid>,
    motion: Option<GeometryMotion>,
) -> GeometryNode {
    GeometryNode {
        id,
        name: name.into(),
        parent_id: parent,
        transform: Transform3::default(),
        pivot: Vector3::default(),
        glb_node: None,
        motion,
    }
}

/// Reuse the graph's Pan node and a Tilt node beneath it, or add nominal coincident ones. Every
/// other rotation the mode binds to Pan/Tilt is dropped: one channel drives one physical axis.
fn motion_nodes(
    profile: &FixtureProfile,
    mode: &FixtureMode,
    graph: &mut GeometryGraph,
    pan: &AxisPlan,
    tilt: &AxisPlan,
) -> (Uuid, Uuid) {
    let pan_attribute = mode.channels[pan.channel].attribute.clone();
    let tilt_attribute = mode.channels[tilt.channel].attribute.clone();
    let bound = |n: &GeometryNode, attribute: &AttributeKey| {
        n.motion.as_ref().is_some_and(|m| {
            m.kind == GeometryMotionKind::Rotation && m.attribute.as_ref() == Some(attribute)
        })
    };
    let descends = |graph: &GeometryGraph, id: Uuid, ancestor: Uuid| {
        let mut cursor = graph
            .nodes
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.parent_id);
        while let Some(parent) = cursor {
            if parent == ancestor {
                return true;
            }
            cursor = graph
                .nodes
                .iter()
                .find(|n| n.id == parent)
                .and_then(|n| n.parent_id);
        }
        false
    };
    let reused = graph
        .nodes
        .iter()
        .find(|n| bound(n, &pan_attribute))
        .map(|n| n.id)
        .and_then(|pan_id| {
            graph
                .nodes
                .iter()
                .find(|n| bound(n, &tilt_attribute) && descends(graph, n.id, pan_id))
                .map(|n| (pan_id, n.id))
        });
    let (pan_id, tilt_id) = reused.unwrap_or_else(|| {
        (
            derived_id("pan-node", profile.id.0, mode.id),
            derived_id("tilt-node", profile.id.0, mode.id),
        )
    });
    for n in &mut graph.nodes {
        if n.id != pan_id
            && n.id != tilt_id
            && n.motion.as_ref().is_some_and(|m| {
                m.attribute.as_ref() == Some(&pan_attribute)
                    || m.attribute.as_ref() == Some(&tilt_attribute)
            })
        {
            n.motion = None;
        }
    }
    if reused.is_some() {
        for n in &mut graph.nodes {
            if let Some(m) = n
                .motion
                .as_mut()
                .filter(|_| n.id == pan_id || n.id == tilt_id)
            {
                let (attribute, range) = if n.id == pan_id {
                    (&pan_attribute, pan.range)
                } else {
                    (&tilt_attribute, tilt.range)
                };
                *m = GeometryMotion {
                    max_speed_per_second: m.max_speed_per_second,
                    acceleration_per_second_squared: m.acceleration_per_second_squared,
                    deceleration_per_second_squared: m.deceleration_per_second_squared,
                    ..motion(&attribute.0, m.axis, range)
                };
            }
        }
        return (pan_id, tilt_id);
    }
    let root = match graph.nodes.iter().find(|n| n.parent_id.is_none()) {
        Some(root) => root.id,
        None => {
            let id = derived_id("body-node", profile.id.0, mode.id);
            graph.nodes.push(node(id, "Body", None, None));
            id
        }
    };
    let y = Vector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    let x = Vector3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    graph.nodes.push(node(
        pan_id,
        "Derived Pan",
        Some(root),
        Some(motion(&pan_attribute.0, y, pan.range)),
    ));
    graph.nodes.push(node(
        tilt_id,
        "Derived Tilt",
        Some(pan_id),
        Some(motion(&tilt_attribute.0, x, tilt.range)),
    ));
    (pan_id, tilt_id)
}

fn install(mode: &mut FixtureMode, plan: Plan) {
    for (axis, role, node) in [
        (&plan.pan, PositionAxisRole::Pan, plan.pan_node),
        (&plan.tilt, PositionAxisRole::Tilt, plan.tilt_node),
    ] {
        let channel = &mut mode.channels[axis.channel];
        let mapping = PhysicalMappingCalibration {
            quality: PhysicalDataQuality::Unknown,
            source: Some(
                if axis.declared {
                    DECLARED_MAPPING_SOURCE
                } else {
                    NOMINAL_MAPPING_SOURCE
                }
                .into(),
            ),
            revision: 1,
            samples: Vec::new(),
            opening_convention: None,
        };
        let behavior = ChannelFunctionBehavior::Continuous {
            physical_min: axis.range.0,
            physical_max: axis.range.1,
            unit: Some("degrees".into()),
        };
        let angular = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: None,
            acceleration_degrees_per_second_squared: None,
            deceleration_degrees_per_second_squared: None,
        });
        match axis.function {
            Some(index) => {
                let function = &mut channel.functions[index];
                function.behavior = behavior;
                function.physical_mapping = Some(mapping);
                function.angular_motion = angular;
            }
            None => channel.functions.push(ChannelFunction {
                id: axis.function_id,
                name: channel.attribute.0.to_string(),
                dmx_from: 0,
                dmx_to: channel.resolution.max_raw(),
                attribute: channel.attribute.clone(),
                priority: 0,
                physical_mapping: Some(mapping),
                angular_motion: angular,
                behavior,
            }),
        }
        if channel.functions.len() == 1 {
            channel.physical_min = Some(axis.range.0.min(axis.range.1));
            channel.physical_max = Some(axis.range.0.max(axis.range.1));
            channel.unit = Some("degrees".into());
        }
        let binding = MotionFunctionBinding {
            node_id: node,
            channel_id: channel.id,
            function_id: axis.function_id,
            role,
        };
        mode.position_physical
            .get_or_insert_with(|| PositionPhysicalModel {
                version: 1,
                revision: 1,
                bindings: Vec::new(),
            })
            .bindings
            .push(binding);
    }
    mode.geometry = plan.graph;
}
