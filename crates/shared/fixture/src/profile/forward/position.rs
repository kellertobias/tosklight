use crate::{
    AngularMotionKind, CompiledPhysicalMapping, EffectiveAxisCalibration, FixtureMode,
    FixtureProfile, GeometryBracket, GeometryGraph, GeometryMotion, GeometryMotionKind,
    GeometryNode, InstalledPositionCalibration, MotionFunctionBinding, PhysicalDataQuality,
    PositionAxisRole, PositionCalibrationContext, ProfileError, Vector3,
};
use light_core::spatial::RigidTransform as R;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default)]
pub struct PositionInstallation<'a> {
    pub calibration: Option<&'a InstalledPositionCalibration>,
    pub invert_pan: bool,
    pub invert_tilt: bool,
    pub bracket_degrees: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForwardMotionLimits {
    pub speed: Option<f64>,
    pub acceleration: Option<f64>,
    pub deceleration: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalAxisValue {
    pub function_id: Uuid,
    pub value: f64,
    pub limits: ForwardMotionLimits,
    pub quality: PhysicalDataQuality,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AxisForwardCommand {
    pub node_id: Uuid,
    pub role: Option<PositionAxisRole>,
    /// Both Some means ambiguous simultaneous authority, including zero angular velocity.
    pub absolute: Option<PhysicalAxisValue>,
    pub velocity: Option<PhysicalAxisValue>,
}
impl AxisForwardCommand {
    pub fn absolute_degrees(&self) -> Option<f64> {
        self.absolute
            .filter(|_| self.velocity.is_none())
            .map(|a| a.value)
    }
    pub fn conflict(&self) -> bool {
        self.absolute.is_some() && self.velocity.is_some()
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PositionForwardFlags(pub u8);
impl PositionForwardFlags {
    pub const STALE_CALIBRATION: Self = Self(1);
    pub const UNSUPPORTED_BRACKET: Self = Self(2);
    pub const UNKNOWN_AXIS: Self = Self(4);
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct LensForwardPose {
    pub emitter_id: Uuid,
    pub head_id: Option<Uuid>,
    /// None means unknown commanded pose, not a request to draw the neutral pose as achieved.
    pub local: Option<R>,
    pub world: Option<R>,
    pub flags: PositionForwardFlags,
    pub geometry_quality: PhysicalDataQuality,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionForwardInputError {
    ChannelCount,
    RawOutOfRange,
    OutputLayout,
    AxisCount,
    NonfiniteAxis,
}
#[derive(Clone, Debug)]
pub struct PositionForwardWorkspace {
    plan_id: Uuid,
    nodes: Vec<Option<R>>,
}
#[derive(Clone, Debug)]
pub struct CompiledPositionForward {
    maxima: Box<[u32]>,
    axes: Box<[Axis]>,
    geometry: std::sync::Arc<PositionPoseGraph>,
}
/// Immutable renderer-independent geometry, transferable once per scene configuration.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PositionPoseGraph {
    plan_id: Uuid,
    axis_count: usize,
    nodes: Box<[Node]>,
    lenses: Box<[Lens]>,
    flags: PositionForwardFlags,
    geometry_quality: PhysicalDataQuality,
}
#[derive(Clone, Debug)]
pub(in crate::profile) struct Driver {
    pub(in crate::profile) channel: usize,
    pub(in crate::profile) from: u32,
    pub(in crate::profile) to: u32,
    pub(in crate::profile) mapping: CompiledPhysicalMapping,
    pub(in crate::profile) velocity: bool,
    pub(in crate::profile) limits: ForwardMotionLimits,
}
#[derive(Clone, Debug)]
pub(in crate::profile) struct Axis {
    pub(in crate::profile) node_id: Uuid,
    pub(in crate::profile) role: Option<PositionAxisRole>,
    pub(in crate::profile) calibration: EffectiveAxisCalibration,
    pub(in crate::profile) calibration_quality: Option<PhysicalDataQuality>,
    pub(in crate::profile) drivers: Box<[Driver]>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Node {
    id: Uuid,
    parent: Option<usize>,
    before: R,
    after: R,
    axis: Option<usize>,
    direction: [f64; 3],
    translation: bool,
    neutral_world: R,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Lens {
    id: Uuid,
    head: Option<Uuid>,
    node: usize,
    local: R,
}
fn invalid(s: impl Into<String>) -> ProfileError {
    ProfileError::Invalid(format!("Position forward model: {}", s.into()))
}
fn vector(v: Vector3) -> [f64; 3] {
    [f64::from(v.x), f64::from(v.y), f64::from(v.z)]
}
fn metres(v: Vector3) -> [f64; 3] {
    vector(v).map(|v| v / 1000.)
}

/// Order every physical ancestor of emitters and moving nodes parent-first.
fn physical_ancestry(
    graph: &GeometryGraph,
) -> Result<(HashMap<Uuid, usize>, Vec<&GeometryNode>), ProfileError> {
    let source_nodes: HashMap<_, _> = graph.nodes.iter().map(|n| (n.id, n)).collect();
    // Include every physical ancestor, independent of GLB artwork. Decorative scaled nodes
    // outside physical ancestry belong to the renderer's static model, not this rigid model.
    let mut needed = HashSet::new();
    for start in graph.emitters.iter().map(|e| e.node_id).chain(
        graph
            .nodes
            .iter()
            .filter(|n| n.motion.is_some())
            .map(|n| n.id),
    ) {
        let mut cursor = Some(start);
        let mut seen = HashSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return Err(invalid("cyclic ancestry"));
            }
            needed.insert(id);
            cursor = source_nodes
                .get(&id)
                .ok_or_else(|| invalid("missing ancestor"))?
                .parent_id;
        }
    }
    let mut indices = HashMap::new();
    let mut order = Vec::new();
    while order.len() < needed.len() {
        let before = order.len();
        for n in &graph.nodes {
            if needed.contains(&n.id)
                && !indices.contains_key(&n.id)
                && n.parent_id.is_none_or(|id| indices.contains_key(&id))
            {
                indices.insert(n.id, order.len());
                order.push(n);
            }
        }
        if order.len() == before {
            return Err(invalid("unresolved ancestry"));
        }
    }
    Ok((indices, order))
}

/// Compile the native drivers of one moving node with their effective motion limits.
fn compile_drivers(
    mode: &FixtureMode,
    bindings: &[MotionFunctionBinding],
    channel_indices: &HashMap<Uuid, usize>,
    node_id: Uuid,
    motion: &GeometryMotion,
) -> Result<Box<[Driver]>, ProfileError> {
    bindings
        .iter()
        .filter(|b| b.node_id == node_id)
        .map(|binding| {
            let channel = channel_indices[&binding.channel_id];
            let c = &mode.channels[channel];
            let f = c
                .functions
                .iter()
                .find(|f| f.id == binding.function_id)
                .unwrap();
            let a = f.angular_motion.unwrap();
            if [
                a.max_speed_degrees_per_second,
                a.acceleration_degrees_per_second_squared,
                a.deceleration_degrees_per_second_squared,
                motion.max_speed_per_second,
                motion.acceleration_per_second_squared,
                motion.deceleration_per_second_squared,
            ]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v <= 0.)
            {
                return Err(invalid("motion limits must be finite and positive"));
            }
            Ok(Driver {
                channel,
                from: f.dmx_from,
                to: f.dmx_to,
                mapping: CompiledPhysicalMapping::compile(c, f)?.unwrap(),
                velocity: a.kind == AngularMotionKind::AngularVelocity,
                limits: ForwardMotionLimits {
                    speed: a
                        .max_speed_degrees_per_second
                        .or(motion.max_speed_per_second)
                        .map(f64::from),
                    acceleration: a
                        .acceleration_degrees_per_second_squared
                        .or(motion.acceleration_per_second_squared)
                        .map(f64::from),
                    deceleration: a
                        .deceleration_degrees_per_second_squared
                        .or(motion.deceleration_per_second_squared)
                        .map(f64::from),
                },
            })
        })
        .collect::<Result<Box<[_]>, ProfileError>>()
}

fn compile_lenses(graph: &GeometryGraph, indices: &HashMap<Uuid, usize>) -> Box<[Lens]> {
    graph
        .emitters
        .iter()
        .map(|e| Lens {
            id: e.id,
            head: e.head_id,
            node: indices[&e.node_id],
            local: R::translation(metres(e.origin))
                .unwrap()
                .compose(R::euler_xyz(vector(e.orientation_degrees)).unwrap()),
        })
        .collect()
}

/// A node's neutral and bracket-adjusted transforms around its pivot.
struct NodeTransforms {
    neutral_rotation: R,
    neutral_before: R,
    before: R,
    after: R,
}

fn node_transforms(
    n: &GeometryNode,
    bracket: &GeometryBracket,
    bracket_degrees: f64,
) -> NodeTransforms {
    let neutral_rotation = R::euler_xyz(vector(n.transform.rotation_degrees)).unwrap();
    let neutral_before = R::translation(metres(n.transform.translation))
        .unwrap()
        .compose(R::translation(metres(n.pivot)).unwrap())
        .compose(neutral_rotation);
    let mut before = neutral_before;
    if let GeometryBracket::Hinge {
        node_id,
        pivot,
        axis,
    } = *bracket
        && node_id == n.id
    {
        before = R::about_pivot(
            R::axis_angle(vector(axis), bracket_degrees).unwrap(),
            metres(pivot),
        )
        .unwrap()
        .compose(before);
    }
    let after = R::translation(metres(n.pivot).map(|v| -v)).unwrap();
    NodeTransforms {
        neutral_rotation,
        neutral_before,
        before,
        after,
    }
}

impl CompiledPositionForward {
    /// All profile identities, ancestry and calibration are compiled on configuration changes.
    /// None asks the caller to retain an explicitly nominal legacy representation.
    pub fn compile(
        profile: &FixtureProfile,
        mode_id: Uuid,
        installed: PositionInstallation<'_>,
    ) -> Result<Option<Self>, ProfileError> {
        let mode = profile
            .mode(mode_id)
            .ok_or_else(|| invalid("missing mode"))?;
        let graph = profile.mode_geometry(mode);
        let Some(contract) = &graph.physical_contract else {
            return Ok(None);
        };
        graph.validate(&mode.heads.iter().map(|h| h.id).collect())?;
        mode.validate_position_physical(&graph)?;
        super::validate_native_domains(mode)?;
        if !installed.bracket_degrees.is_finite() || mode.channels.len() > 4096 {
            return Err(invalid("invalid bracket or mode capacity"));
        }
        if let Some(c) = installed.calibration {
            c.validate().map_err(invalid)?;
        }
        let context = PositionCalibrationContext::new(profile, mode_id).map_err(invalid)?;
        let mut calibration = installed.calibration.cloned().unwrap_or_default();
        let mut flags = PositionForwardFlags::default();
        if calibration.axis_overrides.as_ref().is_some_and(|c| {
            context
                .as_ref()
                .is_none_or(|context| c.validate_for_context(context).is_err())
        }) {
            flags.0 |= PositionForwardFlags::STALE_CALIBRATION.0;
            calibration.axis_overrides = None;
        }
        if installed.bracket_degrees != 0.
            && !matches!(contract.bracket, GeometryBracket::Hinge { .. })
        {
            flags.0 |= PositionForwardFlags::UNSUPPORTED_BRACKET.0;
        }
        let (indices, order) = physical_ancestry(&graph)?;
        let channel_indices: HashMap<_, _> = mode
            .channels
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let bindings = mode
            .position_physical
            .as_ref()
            .map_or(&[][..], |m| m.bindings.as_slice());
        let mut axes = Vec::new();
        let mut nodes: Vec<Node> = Vec::new();
        for n in order {
            let parent = n.parent_id.map(|id| indices[&id]);
            let NodeTransforms {
                neutral_rotation,
                neutral_before,
                before,
                after,
            } = node_transforms(n, &contract.bracket, installed.bracket_degrees);
            let axis_index = n.motion.as_ref().map(|_| axes.len());
            let mut direction = [0., 1., 0.];
            let mut translation = false;
            if let Some(motion) = &n.motion {
                direction = vector(motion.axis);
                let len = direction[0].hypot(direction[1]).hypot(direction[2]);
                direction = direction.map(|v| v / len);
                translation = motion.kind == GeometryMotionKind::Translation;
                if translation {
                    direction = neutral_rotation.inverse().direction(direction);
                }
                let role = bindings.iter().find(|b| b.node_id == n.id).map(|b| b.role);
                let correction = match (context.as_ref(), role) {
                    (Some(c), Some(role)) => calibration
                        .effective_axis(c, n.id, role, installed.invert_pan, installed.invert_tilt)
                        .map_err(invalid)?,
                    _ => EffectiveAxisCalibration {
                        zero_degrees: 0.,
                        invert: false,
                    },
                };
                let drivers = compile_drivers(mode, bindings, &channel_indices, n.id, motion)?;
                axes.push(Axis {
                    node_id: n.id,
                    role,
                    calibration: correction,
                    calibration_quality: installed.calibration.map(|c| c.quality),
                    drivers,
                });
            }
            let neutral_world = parent
                .map_or(R::IDENTITY, |i| nodes[i].neutral_world)
                .compose(neutral_before.compose(after));
            nodes.push(Node {
                id: n.id,
                parent,
                before,
                after,
                axis: axis_index,
                direction,
                translation,
                neutral_world,
            });
        }
        let lenses = compile_lenses(&graph, &indices);
        Ok(Some(Self {
            maxima: mode
                .channels
                .iter()
                .map(|c| c.resolution.max_raw())
                .collect(),
            geometry: std::sync::Arc::new(PositionPoseGraph {
                plan_id: Uuid::new_v4(),
                axis_count: axes.len(),
                nodes: nodes.into_boxed_slice(),
                lenses,
                flags,
                geometry_quality: contract.provenance.quality,
            }),
            axes: axes.into_boxed_slice(),
        }))
    }
    pub(in crate::profile) fn fitting_axes(&self) -> &[Axis] {
        &self.axes
    }

    /// Configuration-only ancestry compilation for inverse candidate evaluation.
    pub(in crate::profile) fn fitting_ancestry(&self, lens: usize) -> Box<[usize]> {
        let mut path = Vec::new();
        let mut node = self.geometry.lenses.get(lens).map(|l| l.node);
        while let Some(i) = node {
            path.push(i);
            node = self.geometry.nodes[i].parent;
        }
        path.reverse();
        path.into_boxed_slice()
    }
    /// One candidate ray, using the same local transforms as evaluate_pose. The chain is supplied
    /// only by the immutable inverse compiler; unrelated heads/nodes are not evaluated per trial.
    pub(in crate::profile) fn fitting_lens_geometry(
        &self,
        lens: usize,
        chain: &[usize],
        axes: &[Option<f64>],
        mount: R,
        pair: [usize; 2],
    ) -> Option<(R, [AxisTangent; 2])> {
        if self
            .geometry
            .flags
            .contains(PositionForwardFlags::UNSUPPORTED_BRACKET)
        {
            return None;
        }
        let mut pose = mount;
        let mut tangents = [None; 2];
        for &i in chain {
            let node = &self.geometry.nodes[i];
            if let Some(j) = node.axis.and_then(|a| pair.iter().position(|&p| p == a)) {
                if node.translation {
                    return None;
                }
                let before = pose.compose(node.before);
                let length = node.direction[0]
                    .hypot(node.direction[1])
                    .hypot(node.direction[2]);
                tangents[j] = Some(AxisTangent {
                    radians_per_degree: before
                        .direction(node.direction.map(|v| v / length))
                        .map(|v| v * std::f64::consts::PI / 180.),
                    pivot: before.point([0.; 3]),
                });
            }
            pose = pose.compose(node.local_pose(axes)?);
        }
        Some((
            pose.compose(self.geometry.lenses[lens].local),
            [tangents[0]?, tangents[1]?],
        ))
    }
    pub fn inputs_available(&self, axis: usize, available: &[bool]) -> bool {
        self.axes.get(axis).is_some_and(|a| {
            a.drivers
                .iter()
                .all(|d| available.get(d.channel) == Some(&true))
        })
    }
    pub fn create_commands(&self) -> Vec<AxisForwardCommand> {
        self.axes
            .iter()
            .map(|a| AxisForwardCommand {
                node_id: a.node_id,
                role: a.role,
                absolute: None,
                velocity: None,
            })
            .collect()
    }
    pub fn decode_commands(
        &self,
        raw: &[u32],
        output: &mut [AxisForwardCommand],
    ) -> Result<(), PositionForwardInputError> {
        if raw.len() != self.maxima.len() {
            return Err(PositionForwardInputError::ChannelCount);
        }
        if raw.iter().zip(&self.maxima).any(|(r, m)| r > m) {
            return Err(PositionForwardInputError::RawOutOfRange);
        }
        if output.len() != self.axes.len()
            || output
                .iter()
                .zip(&self.axes)
                .any(|(o, a)| o.node_id != a.node_id || o.role != a.role)
        {
            return Err(PositionForwardInputError::OutputLayout);
        }
        for (axis, out) in self.axes.iter().zip(output) {
            out.absolute = None;
            out.velocity = None;
            for d in axis
                .drivers
                .iter()
                .filter(|d| (d.from..=d.to).contains(&raw[d.channel]))
            {
                let physical = d.mapping.physical_for_raw(raw[d.channel]).physical;
                let value = PhysicalAxisValue {
                    function_id: d.mapping.function_id,
                    value: if d.velocity {
                        axis.calibration.sign() * physical
                    } else {
                        axis.calibration.physical_to_calibrated(physical)
                    },
                    limits: d.limits,
                    quality: axis.calibration_quality.map_or(d.mapping.quality, |q| {
                        super::color::quality_min(d.mapping.quality, q)
                    }),
                };
                if d.velocity {
                    out.velocity = Some(value);
                } else {
                    out.absolute = Some(value);
                }
            }
        }
        Ok(())
    }
    /// Calibrated unwrapped angles: immediate commands OR Stage's simulated positions.
    /// The caller owns motion integration. This calculation never delays native output.
    pub fn pose_graph(&self) -> std::sync::Arc<PositionPoseGraph> {
        self.geometry.clone()
    }
    pub fn create_workspace(&self) -> PositionForwardWorkspace {
        self.geometry.create_workspace()
    }
    pub fn create_output(&self) -> Vec<LensForwardPose> {
        self.geometry.create_output()
    }
    pub fn evaluate_pose(
        &self,
        axes: &[Option<f64>],
        mount: R,
        workspace: &mut PositionForwardWorkspace,
        output: &mut [LensForwardPose],
    ) -> Result<(), PositionForwardInputError> {
        self.geometry.evaluate_pose(axes, mount, workspace, output)
    }
    pub fn node_delta(
        &self,
        workspace: &PositionForwardWorkspace,
        index: usize,
    ) -> Option<(Uuid, R)> {
        self.geometry.node_delta(workspace, index)
    }
    pub fn node_count(&self) -> usize {
        self.geometry.node_count()
    }
}
impl PositionPoseGraph {
    /// Configuration-time ancestor dependency list; unrelated articulated heads stay independent.
    pub fn lens_axis_indices(&self, lens: usize) -> Box<[usize]> {
        let mut result = Vec::new();
        let mut node = self.lenses.get(lens).map(|l| l.node);
        while let Some(index) = node {
            if let Some(axis) = self.nodes[index].axis {
                result.push(axis);
            }
            node = self.nodes[index].parent;
        }
        result.into_boxed_slice()
    }

    pub fn id(&self) -> Uuid {
        self.plan_id
    }
    pub fn node_id(&self, index: usize) -> Option<Uuid> {
        self.nodes.get(index).map(|n| n.id)
    }
    pub fn axis_count(&self) -> usize {
        self.axis_count
    }
    pub fn create_workspace(&self) -> PositionForwardWorkspace {
        PositionForwardWorkspace {
            plan_id: self.plan_id,
            nodes: vec![None; self.nodes.len()],
        }
    }
    pub fn create_output(&self) -> Vec<LensForwardPose> {
        self.lenses
            .iter()
            .map(|e| LensForwardPose {
                emitter_id: e.id,
                head_id: e.head,
                local: None,
                world: None,
                flags: self.flags,
                geometry_quality: self.geometry_quality,
            })
            .collect()
    }
    pub fn evaluate_pose(
        &self,
        axes: &[Option<f64>],
        mount: R,
        workspace: &mut PositionForwardWorkspace,
        output: &mut [LensForwardPose],
    ) -> Result<(), PositionForwardInputError> {
        if axes.len() != self.axis_count {
            return Err(PositionForwardInputError::AxisCount);
        }
        if axes.iter().flatten().any(|v| !v.is_finite()) {
            return Err(PositionForwardInputError::NonfiniteAxis);
        }
        if workspace.plan_id != self.plan_id
            || workspace.nodes.len() != self.nodes.len()
            || output.len() != self.lenses.len()
            || output
                .iter()
                .zip(&self.lenses)
                .any(|(a, b)| a.emitter_id != b.id || a.head_id != b.head)
        {
            return Err(PositionForwardInputError::OutputLayout);
        }
        for (index, node) in self.nodes.iter().enumerate() {
            let parent = node
                .parent
                .map_or(Some(R::IDENTITY), |i| workspace.nodes[i]);
            workspace.nodes[index] = parent
                .zip(node.local_pose(axes))
                .map(|(parent, local)| parent.compose(local));
        }
        for (lens, out) in self.lenses.iter().zip(output) {
            out.flags = self.flags;
            out.local = if self
                .flags
                .contains(PositionForwardFlags::UNSUPPORTED_BRACKET)
            {
                None
            } else {
                workspace.nodes[lens.node].map(|n| n.compose(lens.local))
            };
            out.world = out.local.map(|p| mount.compose(p));
            out.geometry_quality = self.geometry_quality;
            if workspace.nodes[lens.node].is_none() {
                out.flags.0 |= PositionForwardFlags::UNKNOWN_AXIS.0;
            }
        }
        Ok(())
    }
    /// Dynamic delta for a GLB node already baked into authored neutral geometry.
    pub fn node_delta(
        &self,
        workspace: &PositionForwardWorkspace,
        index: usize,
    ) -> Option<(Uuid, R)> {
        if workspace.plan_id != self.plan_id
            || self
                .flags
                .contains(PositionForwardFlags::UNSUPPORTED_BRACKET)
        {
            return None;
        }
        let node = self.nodes.get(index)?;
        Some((
            node.id,
            workspace
                .nodes
                .get(index)?
                .as_ref()?
                .compose(node.neutral_world.inverse()),
        ))
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Exact world-space angular derivative of a calibrated graph joint (not installation drive).
#[derive(Clone, Copy, Debug)]
pub(in crate::profile) struct AxisTangent {
    pub(in crate::profile) radians_per_degree: [f64; 3],
    pub(in crate::profile) pivot: [f64; 3],
}

impl Node {
    fn local_pose(&self, axes: &[Option<f64>]) -> Option<R> {
        let dynamic = match self.axis {
            None => Some(R::IDENTITY),
            Some(i) => axes[i].and_then(|value| {
                if self.translation {
                    R::translation(self.direction.map(|v| v * value / 1000.))
                } else {
                    R::axis_angle(self.direction, value)
                }
            }),
        }?;
        Some(self.before.compose(dynamic).compose(self.after))
    }
}
