//! Model-content keyed, current-scene bounded geometry and instance allocations.
use super::super::context::FrameContext;
use super::{Segment, pipelines::Pipelines};
use crate::{
    buffers::{DynamicBuffer, GpuMesh},
    instances::{MeshInstance, MeshKind},
    mesh::{self, MeshData, Vertex},
};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::Hasher,
};
use wgpu::util::DeviceExt;

pub(super) struct Geometry {
    pub mesh: GpuMesh,
    pub edge_group: wgpu::BindGroup,
    pub edge_count: u32,
    pub instances: DynamicBuffer,
    pub instance_bytes: Vec<u8>,
    fingerprint: u64,
}

pub(super) fn ensure(
    retained: &mut HashMap<MeshKind, Geometry>,
    context: &FrameContext<'_>,
    pipelines: &Pipelines,
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
    for (kind, instances) in drawn.iter().filter(|(_, instances)| !instances.is_empty()) {
        let Some(fingerprint) = fingerprint(scene, *kind) else {
            continue;
        };
        if retained
            .get(kind)
            .is_some_and(|mesh| mesh.fingerprint == fingerprint)
        {
            continue;
        }
        let Some(data) = mesh_data(scene, *kind) else {
            continue;
        };
        if data.vertices.is_empty() || data.indices.is_empty() {
            retained.remove(kind);
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
            .collect::<Vec<_>>();
        let empty = [Segment {
            start: [0.0; 4],
            end: [0.0; 4],
            a: [0.0; 4],
            b: [0.0; 4],
        }];
        let buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("retained model feature edges"),
                contents: bytemuck::cast_slice(if edges.is_empty() { &empty } else { &edges }),
                usage: wgpu::BufferUsages::STORAGE,
            });
        retained.insert(
            *kind,
            Geometry {
                mesh: GpuMesh::new(context.device, "wireframe actual geometry", &data),
                edge_group: pipelines.segment_group(context.device, &buffer),
                edge_count: edges.len() as u32,
                instances: DynamicBuffer::new(
                    context.device,
                    "wireframe changed instances",
                    wgpu::BufferUsages::VERTEX,
                    (instances.len().max(1) * size_of::<MeshInstance>()) as u64,
                ),
                instance_bytes: Vec::new(),
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
