//! How a lens follows its Position axes, beyond a rigid moving head.
//!
//! The Position physical graph is a rigid chain by default: every axis turns its node and the
//! lens rides on the last one (a moving head). Two further kinds belong to the same graph, the
//! same forward model and the same fitter (no parallel solver):
//!
//! - **Mirror:** the lamp is fixed in the body and its beam is deflected by a moving mirror (a
//!   mirror scanner). The lens sits on the mirror: its origin is the point the beam strikes and its
//!   local −Y is the mirror normal. The outgoing beam is the incident beam reflected about that
//!   normal, so a mirror turn of δ about an axis across the plane of incidence deflects the beam by
//!   2δ, while a turn about the incident beam turns it by δ. `axis_ratios` say how many mechanical
//!   mirror degrees one joint degree is, so joint angles can describe beam deflection.
//! - **Fixed axes:** an axis the fixture cannot move (the Pan of a Tilt-only fixture). It is a
//!   graph node at a constant angle with no native driver, so the Position family stays a complete
//!   Pan/Tilt pair: Angles keep the moving axis exact and report the fixed one, and a Target is
//!   aimed as closely as the single axis allows, with the remaining angular error reported.
use super::{
    FixtureMode, GeometryGraph, GeometryMotionKind, PositionAxisRole, ProfileError, Vector3,
};
use light_core::spatial::RigidTransform as R;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Absent (the default) is a rigid moving head.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PositionKinematics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror: Option<MirrorKinematics>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed_axes: Vec<FixedPositionAxis>,
}

impl PositionKinematics {
    pub fn is_moving_head(&self) -> bool {
        self.mirror.is_none() && self.fixed_axes.is_empty()
    }
}

/// A beam deflected by a moving mirror.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MirrorKinematics {
    /// The lens on the mirror: origin is where the beam strikes it, local −Y is its normal.
    pub emitter_id: Uuid,
    /// The node carrying the lamp; an ancestor of (or) the mirror lens's node.
    pub source_node_id: Uuid,
    /// Direction of the light arriving at the mirror, in the source node's frame.
    pub incident: Vector3,
    /// Mechanical mirror degrees per joint degree. Axes not listed turn one to one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axis_ratios: Vec<MirrorAxisRatio>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MirrorAxisRatio {
    pub node_id: Uuid,
    pub mechanical_per_degree: f32,
}

/// An axis that does not move: a rotation node held at `degrees`, with no native driver.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixedPositionAxis {
    pub node_id: Uuid,
    pub role: PositionAxisRole,
    pub degrees: f32,
}

fn invalid(s: &str) -> ProfileError {
    ProfileError::Invalid(format!("Position kinematics: {s}"))
}

fn unit(v: Vector3) -> Option<[f64; 3]> {
    let v = [f64::from(v.x), f64::from(v.y), f64::from(v.z)];
    let length = v[0].hypot(v[1]).hypot(v[2]);
    (length.is_finite() && length > 1e-9).then(|| v.map(|c| c / length))
}

fn ancestry(graph: &GeometryGraph, start: Uuid) -> Vec<Uuid> {
    let mut path = Vec::new();
    let mut cursor = Some(start);
    while let Some(id) = cursor {
        if path.contains(&id) {
            break;
        }
        path.push(id);
        cursor = graph
            .nodes
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.parent_id);
    }
    path
}

impl FixtureMode {
    /// Kinematics refer to existing rotation nodes and lenses; a fixed axis is never also driven.
    pub(super) fn validate_position_kinematics(
        &self,
        graph: &GeometryGraph,
        bound: &HashSet<Uuid>,
    ) -> Result<(), ProfileError> {
        let Some(kinematics) = self.position_physical.as_ref().map(|m| &m.kinematics) else {
            return Ok(());
        };
        let rotation = |id: Uuid| {
            graph.nodes.iter().any(|n| {
                n.id == id
                    && n.motion
                        .as_ref()
                        .is_some_and(|m| m.kind == GeometryMotionKind::Rotation)
            })
        };
        let mut fixed = HashSet::new();
        for axis in &kinematics.fixed_axes {
            if !rotation(axis.node_id)
                || bound.contains(&axis.node_id)
                || !fixed.insert(axis.node_id)
                || !axis.degrees.is_finite()
            {
                return Err(invalid(
                    "a fixed axis needs its own undriven rotation node and a finite angle",
                ));
            }
        }
        let Some(mirror) = &kinematics.mirror else {
            return Ok(());
        };
        let lens = graph
            .emitters
            .iter()
            .find(|e| e.id == mirror.emitter_id)
            .ok_or_else(|| invalid("mirror lens is missing"))?;
        let path = ancestry(graph, lens.node_id);
        if !path.contains(&mirror.source_node_id) || unit(mirror.incident).is_none() {
            return Err(invalid(
                "the lamp node must carry the mirror and the incident direction must be nonzero",
            ));
        }
        let mut seen = HashSet::new();
        for ratio in &mirror.axis_ratios {
            if !rotation(ratio.node_id)
                || !path.contains(&ratio.node_id)
                || !seen.insert(ratio.node_id)
                || !ratio.mechanical_per_degree.is_finite()
                || ratio.mechanical_per_degree <= 0.0
                || ratio.mechanical_per_degree > 4.0
            {
                return Err(invalid(
                    "a mirror ratio needs a rotation node carrying the mirror and a ratio in (0, 4]",
                ));
            }
        }
        Ok(())
    }
}

/// Compiled mirror of one lens: the lamp node's index in the forward graph and the lamp's
/// orientation in that node's frame (its −Y is the incident direction).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(in crate::profile) struct CompiledMirror {
    pub(in crate::profile) source: usize,
    pub(in crate::profile) lamp: R,
}

impl CompiledMirror {
    pub(in crate::profile) fn new(source: usize, incident: Vector3) -> Option<Self> {
        let d = unit(incident)?;
        let down = [0., -1., 0.];
        let axis = cross(down, d);
        let lamp = if axis[0].hypot(axis[1]).hypot(axis[2]) > 1e-12 {
            R::axis_angle(axis, dot(down, d).clamp(-1., 1.).acos().to_degrees())?
        } else if d[1] < 0. {
            R::IDENTITY
        } else {
            R::axis_angle([1., 0., 0.], 180.)?
        };
        Some(Self { source, lamp })
    }

    /// The outgoing beam pose: origin on the mirror, −Y along the reflected beam. `mirror` is the
    /// lens pose on the mirror (its −Y the normal); `source` the lamp node's pose, same frame.
    /// The image is mirrored, so one axis is flipped to keep the pose right-handed.
    pub(in crate::profile) fn reflect(self, mirror: R, source: R) -> Option<R> {
        let normal = mirror.direction([0., -1., 0.]);
        let lamp = source.compose(self.lamp);
        let h = |v: [f64; 3]| {
            let k = 2. * dot(v, normal);
            std::array::from_fn(|i| v[i] - k * normal[i])
        };
        let [x, y, z] = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]].map(|a| h(lamp.direction(a)));
        R::from_columns([x.map(|v| -v), y, z], mirror.point([0.; 3]))
    }

    /// Derivative of the reflected beam direction for one joint whose rotation (radians per
    /// degree, world axis) turns the mirror normal and, when `moves_lamp`, the incident beam.
    pub(in crate::profile) fn direction_derivative(
        self,
        mirror: R,
        source: R,
        omega: [f64; 3],
        moves_lamp: bool,
    ) -> [f64; 3] {
        let n = mirror.direction([0., -1., 0.]);
        let d = source.compose(self.lamp).direction([0., -1., 0.]);
        let dn = cross(omega, n);
        let dd = if moves_lamp { cross(omega, d) } else { [0.; 3] };
        let along = dot(dd, n) + dot(d, dn);
        let dn_weight = dot(d, n);
        std::array::from_fn(|i| dd[i] - 2. * (along * n[i] + dn_weight * dn[i]))
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
