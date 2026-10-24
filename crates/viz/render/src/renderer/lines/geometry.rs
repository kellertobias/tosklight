//! Model-content keyed, current-scene bounded CPU geometry. Batch uploads share each asset.
use super::Segment;
use crate::{
    instances::{MeshInstance, MeshKind},
    mesh::{self, MeshData, Vertex},
};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::Hasher,
};

pub(super) struct Geometry {
    pub data: MeshData,
    pub edges: Vec<Segment>,
    fingerprint: u64,
}

pub(super) fn ensure(
    retained: &mut HashMap<MeshKind, Geometry>,
    scene: &viz_scene::Scene,
    drawn: &[(MeshKind, Vec<MeshInstance>)],
) -> bool {
    let previous_count = retained.len();
    retained.retain(|kind, _| {
        drawn
            .iter()
            .any(|(drawn, instances)| drawn == kind && !instances.is_empty())
    });
    let mut changed = retained.len() != previous_count;
    for (kind, _) in drawn.iter().filter(|(_, instances)| !instances.is_empty()) {
        let Some(fingerprint) = fingerprint(scene, *kind) else {
            changed |= retained.remove(kind).is_some();
            continue;
        };
        if retained
            .get(kind)
            .is_some_and(|g| g.fingerprint == fingerprint)
        {
            continue;
        }
        let Some(mut data) = mesh_data(scene, *kind) else {
            continue;
        };
        let valid_indices = data
            .indices
            .chunks_exact(3)
            .filter(|triangle| {
                triangle
                    .iter()
                    .all(|index| (*index as usize) < data.vertices.len())
            })
            .flatten()
            .copied()
            .collect();
        data.indices = valid_indices;
        if data.vertices.is_empty() || data.indices.is_empty() {
            changed |= retained.remove(kind).is_some();
            continue;
        }
        let edges = mesh::feature_edges::EdgeGeometry::from_mesh(&data)
            .edges
            .into_iter()
            .map(|edge| Segment {
                start: [
                    edge.start[0],
                    edge.start[1],
                    edge.start[2],
                    f32::from(edge.always),
                ],
                end: [edge.end[0], edge.end[1], edge.end[2], 0.0],
                a: [edge.normal_a[0], edge.normal_a[1], edge.normal_a[2], 0.0],
                b: [edge.normal_b[0], edge.normal_b[1], edge.normal_b[2], 0.0],
            })
            .collect();
        retained.insert(
            *kind,
            Geometry {
                data,
                edges,
                fingerprint,
            },
        );
        changed = true;
    }
    changed
}

/// Hash actual content rather than mesh slot/revision alone. In-place replacement at the same
/// index is valid for this cache too; old topology cannot survive because a revision was reused.
fn fingerprint(scene: &viz_scene::Scene, kind: MeshKind) -> Option<u64> {
    let mut hash = DefaultHasher::new();
    match kind {
        MeshKind::ModelPart(model, part) => {
            let part = scene.models.get(model as usize)?.parts.get(part as usize)?;
            hash.write(bytemuck::cast_slice(&part.positions));
            hash.write(bytemuck::cast_slice(&part.indices));
        }
        MeshKind::PlanArtwork(index) => {
            let artwork = scene.plan_artwork.get(index as usize)?;
            hash.write(bytemuck::cast_slice(&artwork.vertices));
            hash.write(bytemuck::cast_slice(&artwork.indices));
        }
        _ => hash.write_u8(1), // Each immutable procedural kind has its own HashMap slot.
    }
    Some(hash.finish())
}

fn mesh_data(scene: &viz_scene::Scene, kind: MeshKind) -> Option<MeshData> {
    let (positions, indices) = match kind {
        MeshKind::ModelPart(model, part) => {
            let part = scene.models.get(model as usize)?.parts.get(part as usize)?;
            (&part.positions, &part.indices)
        }
        MeshKind::PlanArtwork(index) => {
            let artwork = scene.plan_artwork.get(index as usize)?;
            (&artwork.vertices, &artwork.indices)
        }
        _ => return mesh::procedural(kind).map(|(_, data)| data),
    };
    Some(MeshData {
        vertices: positions
            .iter()
            .map(|p| Vertex {
                position: *p,
                normal: [0.0, 1.0, 0.0],
                uv: [0.0; 2],
            })
            .collect(),
        indices: indices.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacing_model_content_in_same_slot_and_revision_changes_cache_identity() {
        let mut scene = viz_scene::Scene::default();
        scene.models.push(viz_scene::FixtureModel {
            parts: vec![viz_scene::ModelPart {
                positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                indices: vec![0, 1, 2],
                ..Default::default()
            }],
            ..Default::default()
        });
        let kind = MeshKind::ModelPart(0, 0);
        let before = fingerprint(&scene, kind);
        scene.models[0].parts[0].positions[1][0] = 2.0;
        assert_ne!(before, fingerprint(&scene, kind));
        assert_eq!(scene.revision, 0);
    }
}
