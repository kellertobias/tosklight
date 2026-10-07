//! Shared fixture kinematics evaluated on the display clock, independently of commanded output.
use crate::{PhysicalMotionState, Scene, SceneValues};
use glam::{Mat4, Vec3};
use light_core::spatial::RigidTransform;
use light_fixture::forward::{LensForwardPose, PositionForwardWorkspace, PositionPoseGraph};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PhysicalPositionPlan {
    pub instance_id: Uuid,
    pub fixture_index: usize,
    pub graph: Arc<PositionPoseGraph>,
    pub axis_nodes: Box<[Uuid]>,
    /// Scene emitter -> shared graph lens output. Repeated lenses are deliberately distinct.
    pub emitters: Vec<(usize, usize)>,
    pub model_part_nodes: Vec<Option<usize>>,
    pub lens_axes: Box<[Box<[usize]>]>,
    pub model_scale: f32,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PhysicalAxisState {
    pub node_id: Uuid,
    pub motion: PhysicalMotionState,
    pub known: bool,
    pub has_position: bool,
    pub nominal_limits: bool,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PhysicalPositionValues {
    pub instance_id: Uuid,
    pub axes: Vec<PhysicalAxisState>,
    pub graph_id: Uuid,
    pub node_deltas: Vec<Option<Mat4>>,
    #[serde(skip)]
    runtime: Option<PoseRuntime>,
}
#[derive(Clone, Debug)]
struct PoseRuntime {
    workspace: PositionForwardWorkspace,
    angles: Box<[Option<f64>]>,
    lenses: Vec<LensForwardPose>,
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PhysicalPoseState {
    /// In fixture-local profile metres, already including bracket and all physical ancestors.
    pub local: Option<Mat4>,
    pub flags: u8,
    pub nominal_motion: bool,
}
pub fn rigid_matrix(pose: RigidTransform) -> Mat4 {
    let column = |v| Vec3::from_array(pose.direction(v).map(|v| v as f32)).extend(0.);
    Mat4::from_cols(
        column([1., 0., 0.]),
        column([0., 1., 0.]),
        column([0., 0., 1.]),
        Vec3::from_array(pose.point([0.; 3]).map(|v| v as f32)).extend(1.),
    )
}
impl SceneValues {
    /// Configuration only; held state is matched by stable instance/node identity.
    pub fn reconcile_physical_positions(&mut self, scene: &Scene) {
        if self.physical_positions.len() == scene.physical_positions.len()
            && self
                .physical_positions
                .iter()
                .zip(&scene.physical_positions)
                .all(|(v, p)| {
                    v.instance_id == p.instance_id
                        && v.graph_id == p.graph.id()
                        && v.axes
                            .iter()
                            .map(|a| a.node_id)
                            .eq(p.axis_nodes.iter().copied())
                })
        {
            return;
        }
        let old = std::mem::take(&mut self.physical_positions);
        self.physical_positions = scene
            .physical_positions
            .iter()
            .map(|p| {
                let previous = old.iter().find(|v| v.instance_id == p.instance_id);
                PhysicalPositionValues {
                    instance_id: p.instance_id,
                    graph_id: p.graph.id(),
                    axes: p
                        .axis_nodes
                        .iter()
                        .map(|id| {
                            previous
                                .and_then(|v| v.axes.iter().find(|a| a.node_id == *id))
                                .cloned()
                                .unwrap_or(PhysicalAxisState {
                                    node_id: *id,
                                    motion: Default::default(),
                                    known: false,
                                    has_position: false,
                                    nominal_limits: false,
                                })
                        })
                        .collect(),
                    node_deltas: vec![None; p.graph.node_count()],
                    runtime: None,
                }
            })
            .collect();
    }
    /// Keep renderer-owned workspaces across provider snapshots without per-frame reallocations.
    pub fn take_calibrated_runtime_from(&mut self, previous: &mut Self) {
        for (next, old) in self
            .physical_positions
            .iter_mut()
            .zip(&mut previous.physical_positions)
        {
            if next.instance_id == old.instance_id && next.graph_id == old.graph_id {
                next.runtime = old.runtime.take();
                std::mem::swap(&mut next.node_deltas, &mut old.node_deltas);
            }
        }
    }
    pub fn retain_calibrated_motion_from(&mut self, previous: &Self) {
        fn retain(next: &mut PhysicalPositionValues, old: &PhysicalPositionValues) {
            if next.graph_id == old.graph_id && next.axes.len() == old.axes.len() {
                for (axis, old) in next.axes.iter_mut().zip(&old.axes) {
                    axis.motion.position_degrees = old.motion.position_degrees;
                    axis.motion.velocity_degrees_per_second =
                        old.motion.velocity_degrees_per_second;
                    axis.has_position |= old.has_position;
                }
            } else {
                for axis in &mut next.axes {
                    if let Some(old) = old.axes.iter().find(|v| v.node_id == axis.node_id) {
                        axis.motion.position_degrees = old.motion.position_degrees;
                        axis.motion.velocity_degrees_per_second =
                            old.motion.velocity_degrees_per_second;
                        axis.has_position |= old.has_position;
                    }
                }
            }
        }
        if self.physical_positions.len() == previous.physical_positions.len()
            && self
                .physical_positions
                .iter()
                .zip(&previous.physical_positions)
                .all(|(a, b)| a.instance_id == b.instance_id)
        {
            for (next, old) in self
                .physical_positions
                .iter_mut()
                .zip(&previous.physical_positions)
            {
                retain(next, old);
            }
        } else {
            let by_id: std::collections::HashMap<_, _> = previous
                .physical_positions
                .iter()
                .map(|p| (p.instance_id, p))
                .collect();
            for next in &mut self.physical_positions {
                if let Some(old) = by_id.get(&next.instance_id) {
                    retain(next, old);
                }
            }
        }
    }
    /// No profile parsing, UUID lookup, fitting, or workspace allocation after scene adoption.
    pub fn apply_calibrated_motion(&mut self, scene: &Scene, elapsed: f32) {
        self.reconcile_physical_positions(scene);
        for (state, plan) in self
            .physical_positions
            .iter_mut()
            .zip(&scene.physical_positions)
        {
            let runtime = state.runtime.get_or_insert_with(|| PoseRuntime {
                workspace: plan.graph.create_workspace(),
                angles: vec![None; plan.graph.axis_count()].into_boxed_slice(),
                lenses: plan.graph.create_output(),
            });
            for (axis, angle) in state.axes.iter_mut().zip(&mut runtime.angles) {
                axis.motion.advance(elapsed);
                *angle = axis
                    .has_position
                    .then_some(f64::from(axis.motion.position_degrees));
            }
            if plan
                .graph
                .evaluate_pose(
                    &runtime.angles,
                    RigidTransform::IDENTITY,
                    &mut runtime.workspace,
                    &mut runtime.lenses,
                )
                .is_err()
            {
                continue;
            }
            for (index, delta) in state.node_deltas.iter_mut().enumerate() {
                *delta = plan
                    .graph
                    .node_delta(&runtime.workspace, index)
                    .map(|(_, m)| rigid_matrix(m));
            }
            for &(emitter, lens) in &plan.emitters {
                let Some(value) = self.emitters.get_mut(emitter) else {
                    continue;
                };
                let Some(pose) = runtime.lenses.get(lens) else {
                    continue;
                };
                let dependencies = plan.lens_axes.get(lens).map(Box::as_ref).unwrap_or(&[]);
                let unknown = dependencies
                    .iter()
                    .any(|&i| state.axes.get(i).is_none_or(|a| !a.known));
                let nominal_motion = dependencies
                    .iter()
                    .any(|&i| state.axes.get(i).is_some_and(|a| a.nominal_limits));
                value.physical_pose = Some(PhysicalPoseState {
                    local: pose.local.map(rigid_matrix),
                    flags: pose.flags.0 | if unknown { 4 } else { 0 },
                    nominal_motion,
                });
            }
        }
    }
}

// Provider snapshots carry values, not scratch buffers. Renderer adoption moves its workspace.
impl Clone for PhysicalPositionValues {
    fn clone(&self) -> Self {
        Self {
            instance_id: self.instance_id,
            graph_id: self.graph_id,
            axes: self.axes.clone(),
            node_deltas: self.node_deltas.clone(),
            runtime: None,
        }
    }
}
