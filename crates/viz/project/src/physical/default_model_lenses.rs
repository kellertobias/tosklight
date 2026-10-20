//! Visual rest frames for bodies whose mesh belongs to the renderer, not the fixture package.

use crate::PatchedFixture;
use glam::{Mat4, Quat, Vec3};
use light_fixture::{
    GeometryGraph,
    forward::{LensForwardPose, PositionPoseGraph},
};

pub(super) fn default_model_lenses(
    fixture: &PatchedFixture,
    scene: &viz_scene::Scene,
    fixture_index: usize,
    geometry: &GeometryGraph,
    graph: &PositionPoseGraph,
    emitters: &[(usize, usize)],
    lenses: &[LensForwardPose],
) -> Vec<Option<(usize, Mat4)>> {
    if fixture.profile.model_asset.is_some() {
        return Vec::new();
    }
    let Some(model) = scene.fixtures[fixture_index]
        .model
        .and_then(|i| scene.models.get(i as usize))
    else {
        return Vec::new();
    };
    let Some(anchor) = model.emitter_anchor else {
        return Vec::new();
    };
    let axis = model.emitter_axis.unwrap_or(Vec3::NEG_Y);
    let rest = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::NEG_Y, axis), anchor);
    // Estimated Position graphs may predate the renderer-owned mesh. Keep their authoritative
    // axes, calibration and solver unchanged; move the real glass by the same delta as its body.
    emitters
        .iter()
        .map(|&(_, lens)| {
            let id = lenses.get(lens)?.emitter_id;
            let emitter = geometry.emitters.iter().find(|e| e.id == id)?;
            if !model
                .parts
                .iter()
                .any(|p| p.geometry_node_id == Some(emitter.node_id))
            {
                return None;
            }
            let node =
                (0..graph.node_count()).find(|&i| graph.node_id(i) == Some(emitter.node_id))?;
            Some((node, rest))
        })
        .collect()
}
