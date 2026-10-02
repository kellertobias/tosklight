//! Explicit physical coordinates and exact motion bindings; current normalized output is unchanged.
use super::{
    AngularMotionKind, CompiledPhysicalMapping, FixtureMode, FixtureProfile, GeometryGraph,
    GeometryMotionKind, OpticalProvenance, PhysicalUnit, ProfileError, Vector3,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Version 1: local millimetres, right-handed Y-up, neutral beam -Y; positive
/// rotation is right-handed. Base Euler rotation is Rx * Ry * Rz on column vectors.
/// A node is T(translation) T(pivot) Rbase Raxis(angle) S T(-pivot).
/// Each zero scale component retains the legacy identity-scale convention (1).
/// A bracket rotates its node and descendants in that node's parent frame,
/// before the node's own transform. Unknown bracket is explicit, never an inferred GLB hinge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryPhysicalContract {
    pub version: u16,
    pub provenance: OpticalProvenance,
    pub bracket: GeometryBracket,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeometryBracket {
    Unknown,
    Fixed,
    Hinge {
        node_id: Uuid,
        pivot: Vector3,
        axis: Vector3,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionAxisRole {
    Pan,
    Tilt,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionFunctionBinding {
    pub node_id: Uuid,
    pub channel_id: Uuid,
    pub function_id: Uuid,
    pub role: PositionAxisRole,
}
/// One axis may bind disjoint functions on one absolute-position channel and on
/// one velocity channel. The runtime must arbitrate a simultaneous velocity command;
/// a velocity-only axis has no observable absolute angle and cannot solve a target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PositionPhysicalModel {
    pub version: u16,
    pub revision: u32,
    pub bindings: Vec<MotionFunctionBinding>,
}
/// Physical identity deliberately excludes source archives and unrelated color edits.
/// Mode and profile identity still matter: unrelated templates can share node UUIDs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PositionCalibrationIdentity {
    pub profile_id: Uuid,
    pub mode_id: Uuid,
    pub geometry_digest: String,
}
impl PositionCalibrationIdentity {
    pub fn validate(&self) -> Result<(), String> {
        if self.profile_id.is_nil()
            || self.mode_id.is_nil()
            || self.geometry_digest.len() != 64
            || !self
                .geometry_digest
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("invalid Position calibration identity".into());
        }
        Ok(())
    }
}
fn invalid(s: impl Into<String>) -> ProfileError {
    ProfileError::Invalid(format!("physical geometry: {}", s.into()))
}
fn finite(v: Vector3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}
fn axis(v: Vector3) -> bool {
    finite(v) && (f64::from(v.x).powi(2) + f64::from(v.y).powi(2) + f64::from(v.z).powi(2)) > 1e-18
}
impl GeometryGraph {
    pub(super) fn validate_physical_contract(&self) -> Result<(), ProfileError> {
        let Some(contract) = &self.physical_contract else {
            return Ok(());
        };
        if contract.version != 1
            || self.nodes.is_empty()
            || self.nodes.len() > 4096
            || self.emitters.len() > 4096
        {
            return Err(invalid("unsupported version or graph size"));
        }
        contract.provenance.validate().map_err(invalid)?;
        for node in &self.nodes {
            if node.id.is_nil()
                || !finite(node.transform.translation)
                || !finite(node.transform.rotation_degrees)
                || !finite(node.transform.scale)
                || !finite(node.pivot)
                || [
                    node.transform.scale.x,
                    node.transform.scale.y,
                    node.transform.scale.z,
                ]
                .iter()
                .any(|v| *v < 0.0)
            {
                return Err(invalid(
                    "node transforms require finite vectors and nonnegative scales",
                ));
            }
            if let Some(motion) = &node.motion {
                if !axis(motion.axis)
                    || !motion.physical_min.is_finite()
                    || !motion.physical_max.is_finite()
                {
                    return Err(invalid(
                        "motion axis and physical limits must be finite; axis must be nonzero",
                    ));
                }
            }
        }
        for e in &self.emitters {
            if e.id.is_nil()
                || !finite(e.origin)
                || !finite(e.orientation_degrees)
                || !e.beam_angle_degrees.is_finite()
                || !e.field_angle_degrees.is_finite()
            {
                return Err(invalid(
                    "lens origin, orientation and openings must be finite",
                ));
            }
        }
        if let GeometryBracket::Hinge {
            node_id,
            pivot,
            axis: hinge_axis,
        } = &contract.bracket
        {
            if !self.nodes.iter().any(|n| n.id == *node_id) || !finite(*pivot) || !axis(*hinge_axis)
            {
                return Err(invalid(
                    "bracket requires an existing body node, finite pivot and nonzero axis",
                ));
            }
        }
        // Physical paths must be rigid. Legacy zero means identity, not a collapsed axis.
        for start in self
            .nodes
            .iter()
            .filter(|n| n.motion.is_some())
            .map(|n| n.id)
            .chain(self.emitters.iter().map(|e| e.node_id))
        {
            let mut cursor = Some(start);
            let mut seen = HashSet::new();
            while let Some(id) = cursor {
                if !seen.insert(id) {
                    return Err(invalid("cyclic physical path"));
                }
                let node = self
                    .nodes
                    .iter()
                    .find(|n| n.id == id)
                    .ok_or_else(|| invalid("missing physical ancestor"))?;
                if [
                    node.transform.scale.x,
                    node.transform.scale.y,
                    node.transform.scale.z,
                ]
                .iter()
                .any(|s| *s != 0.0 && *s != 1.0)
                {
                    return Err(invalid(
                        "physical paths require identity scale; bake visual scale into measured geometry",
                    ));
                }
                cursor = node.parent_id;
            }
        }
        Ok(())
    }
}
impl FixtureMode {
    pub fn validate_position_physical(&self, graph: &GeometryGraph) -> Result<(), ProfileError> {
        let Some(model) = &self.position_physical else {
            return Ok(());
        };
        if model.version != 1
            || model.bindings.is_empty()
            || model.bindings.len() > 4096
            || graph.physical_contract.is_none()
        {
            return Err(invalid(
                "Position requires version 1, exact bindings and an explicit geometry contract",
            ));
        }
        graph.validate_physical_contract()?;
        let mut seen = HashSet::new();
        let mut axis_roles = HashMap::new();
        let mut drivers = HashMap::new();
        for binding in &model.bindings {
            let node = graph
                .nodes
                .iter()
                .find(|n| n.id == binding.node_id)
                .ok_or_else(|| invalid("Position binding references missing node"))?;
            if !node
                .motion
                .as_ref()
                .is_some_and(|m| m.kind == GeometryMotionKind::Rotation)
            {
                return Err(invalid("Position binds rotational axes only"));
            }
            if !seen.insert((binding.node_id, binding.channel_id, binding.function_id)) {
                return Err(invalid("duplicate Position function binding"));
            }
            if axis_roles
                .insert(binding.node_id, binding.role)
                .is_some_and(|r| r != binding.role)
            {
                return Err(invalid("one physical axis cannot be both Pan and Tilt"));
            }
            let channel = self
                .channels
                .iter()
                .find(|c| c.id == binding.channel_id)
                .ok_or_else(|| invalid("Position binding references missing channel"))?;
            if channel.behavior == super::ChannelBehavior::Static {
                return Err(invalid("static channels cannot drive Position"));
            }
            let shared = self
                .heads
                .iter()
                .find(|h| h.id == channel.head_id)
                .is_some_and(|h| h.master_shared);
            for emitter in &graph.emitters {
                let mut cursor = Some(emitter.node_id);
                let mut seen = HashSet::new();
                while let Some(id) = cursor {
                    if !seen.insert(id) {
                        return Err(invalid("cyclic emitter ancestry"));
                    }
                    if id == node.id
                        && emitter.head_id.is_some_and(|h| h != channel.head_id)
                        && !shared
                    {
                        return Err(invalid(
                            "cross-head motion needs an explicitly shared channel head",
                        ));
                    }
                    cursor = graph
                        .nodes
                        .iter()
                        .find(|n| n.id == id)
                        .and_then(|n| n.parent_id);
                }
            }
            let function = channel
                .functions
                .iter()
                .find(|f| f.id == binding.function_id)
                .ok_or_else(|| invalid("Position binding references missing function"))?;
            let motion = function.angular_motion.ok_or_else(|| {
                invalid("Position function must declare absolute angle or angular velocity")
            })?;
            let mapping =
                CompiledPhysicalMapping::compile(channel, function)?.ok_or_else(|| {
                    invalid("Position function requires a continuous physical mapping")
                })?;
            let velocity = motion.kind == AngularMotionKind::AngularVelocity;
            let expected = if velocity {
                PhysicalUnit::DegreesPerSecond
            } else {
                PhysicalUnit::Degrees
            };
            if mapping.unit != expected {
                return Err(invalid(
                    "Position function unit disagrees with its motion kind",
                ));
            }
            if drivers
                .insert((binding.node_id, velocity), channel.id)
                .is_some_and(|id| id != channel.id)
            {
                return Err(invalid(
                    "one axis cannot have multiple independent channels for the same motion kind",
                ));
            }
        }
        Ok(())
    }
}
impl FixtureProfile {
    pub fn position_calibration_identity(
        &self,
        mode_id: Uuid,
    ) -> Result<Option<PositionCalibrationIdentity>, ProfileError> {
        let mode = self
            .modes
            .iter()
            .find(|m| m.id == mode_id)
            .ok_or_else(|| invalid("missing mode"))?;
        if mode.position_physical.is_none() {
            return Ok(None);
        }
        let graph = self.mode_geometry(mode);
        mode.validate_position_physical(&graph)?;
        // Names, artwork, evidence labels, preview ranges and unrelated Color/Focus
        // functions cannot change a motor zero. Hash only the physical interpretation.
        let nodes=graph.nodes.iter().map(|n|(n.id,serde_json::json!({"parent":n.parent_id,"transform":n.transform,"pivot":n.pivot,"motion":n.motion.as_ref().map(|m|serde_json::json!({"kind":m.kind,"axis":m.axis}))}))).collect::<std::collections::BTreeMap<_,_>>();
        let lenses=graph.emitters.iter().map(|e|(e.id,serde_json::json!({"node":e.node_id,"head":e.head_id,"origin":e.origin,"orientation":e.orientation_degrees,"directional":e.directional,"layout":e.layout}))).collect::<std::collections::BTreeMap<_,_>>();
        let mut bindings = mode
            .position_physical
            .as_ref()
            .unwrap()
            .bindings
            .iter()
            .collect::<Vec<_>>();
        bindings.sort_by_key(|b| (b.node_id, b.channel_id, b.function_id));
        let controls=bindings.iter().map(|b|{
            let c=mode.channels.iter().find(|c|c.id==b.channel_id).unwrap();
            let f=c.functions.iter().find(|f|f.id==b.function_id).unwrap();
            serde_json::json!({"binding":b,"head":c.head_id,"resolution":c.resolution,"invert":c.invert,"transform":c.canonical_transform,"function": {"from":f.dmx_from,"to":f.dmx_to,"behavior":f.behavior,"samples":f.physical_mapping.as_ref().map(|m|&m.samples),"angular":f.angular_motion.map(|a|a.kind)}})
        }).collect::<Vec<_>>();
        let canonical = serde_json::json!({"nodes":nodes,"lenses":lenses,"coordinate_version":graph.physical_contract.as_ref().unwrap().version,"bracket":graph.physical_contract.as_ref().unwrap().bracket,"controls":controls});
        let bytes = serde_json::to_vec(&canonical).map_err(|e| invalid(e.to_string()))?;
        Ok(Some(PositionCalibrationIdentity {
            profile_id: self.id.0,
            mode_id,
            geometry_digest: format!("{:x}", Sha256::digest(bytes)),
        }))
    }
}

impl GeometryGraph {
    /// Offline reference implementation for contract verification. Consumers should compile
    /// ancestry and bindings once before using this math at frame rate (TL-546).
    /// Returns a lens pose in profile-local metres, independent of GLB artwork.
    pub fn reference_lens_pose(
        &self,
        emitter_id: Uuid,
        physical_axes: &HashMap<Uuid, f64>,
        bracket_degrees: f64,
    ) -> Result<light_core::spatial::RigidTransform, ProfileError> {
        use light_core::spatial::RigidTransform as R;
        self.validate_physical_contract()?;
        let contract = self
            .physical_contract
            .as_ref()
            .ok_or_else(|| invalid("geometry contract is unknown"))?;
        if !bracket_degrees.is_finite() {
            return Err(invalid("bracket angle must be finite"));
        }
        for (id, value) in physical_axes {
            if !value.is_finite() || !self.nodes.iter().any(|n| n.id == *id && n.motion.is_some()) {
                return Err(invalid("unknown or nonfinite physical axis"));
            }
        }
        if bracket_degrees != 0.0 && !matches!(contract.bracket, GeometryBracket::Hinge { .. }) {
            return Err(invalid("nonzero bracket requires an authored hinge"));
        }
        let e = self
            .emitters
            .iter()
            .find(|e| e.id == emitter_id)
            .ok_or_else(|| invalid("unknown lens"))?;
        let vec = |v: Vector3| [f64::from(v.x), f64::from(v.y), f64::from(v.z)];
        let mm = |v: Vector3| vec(v).map(|c| c / 1000.0);
        let mut chain = Vec::new();
        let mut cursor = Some(e.node_id);
        let mut seen = HashSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return Err(invalid("cyclic ancestry"));
            }
            let n = self
                .nodes
                .iter()
                .find(|n| n.id == id)
                .ok_or_else(|| invalid("missing ancestor"))?;
            chain.push(n);
            cursor = n.parent_id;
        }
        let mut world = R::IDENTITY;
        for n in chain.into_iter().rev() {
            let mut rotation = R::euler_xyz(vec(n.transform.rotation_degrees))
                .ok_or_else(|| invalid("invalid neutral rotation"))?;
            let mut translation = mm(n.transform.translation);
            if let Some(q) = physical_axes.get(&n.id) {
                let motion = n.motion.as_ref().unwrap();
                match motion.kind {
                    GeometryMotionKind::Rotation => {
                        rotation = rotation.compose(
                            R::axis_angle(vec(motion.axis), *q)
                                .ok_or_else(|| invalid("invalid physical rotation"))?,
                        )
                    }
                    GeometryMotionKind::Translation => {
                        let a = vec(motion.axis);
                        let len = a[0].hypot(a[1]).hypot(a[2]);
                        for i in 0..3 {
                            translation[i] += a[i] / len * q / 1000.0;
                        }
                    }
                }
            }
            let local = R::translation(translation)
                .ok_or_else(|| invalid("nonfinite translated axis"))?
                .compose(R::about_pivot(rotation, mm(n.pivot)).unwrap());
            if let GeometryBracket::Hinge {
                node_id,
                pivot,
                axis,
            } = &contract.bracket
                && *node_id == n.id
            {
                world = world.compose(
                    R::about_pivot(
                        R::axis_angle(vec(*axis), bracket_degrees)
                            .ok_or_else(|| invalid("invalid bracket rotation"))?,
                        mm(*pivot),
                    )
                    .unwrap(),
                );
            }
            world = world.compose(local);
        }
        Ok(world
            .compose(R::translation(mm(e.origin)).unwrap())
            .compose(R::euler_xyz(vec(e.orientation_degrees)).unwrap()))
    }
}
