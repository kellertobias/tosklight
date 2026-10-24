//! Exact position-only reuse for real-model depth vertices; never simplification or quantization.
use crate::mesh::MeshData;
use std::collections::HashMap;

pub(super) fn deduplicate_positions(data: &mut MeshData) {
    // ensure() already validates indices. Keep the helper transactional for malformed callers.
    if data
        .indices
        .iter()
        .any(|&index| index as usize >= data.vertices.len())
    {
        return;
    }
    let mut retained = Vec::with_capacity(data.vertices.len());
    let mut remap = Vec::with_capacity(data.vertices.len());
    let mut positions = HashMap::new();
    for vertex in &data.vertices {
        // +/-0 are the same geometric position. Nonfinite vertices are deliberately not merged;
        // this optimization must not reinterpret malformed geometry or NaN payload identity.
        let key = vertex.position.iter().all(|v| v.is_finite()).then(|| {
            vertex
                .position
                .map(|value| if value == 0.0 { 0 } else { value.to_bits() })
        });
        if let Some(index) = key.and_then(|key| positions.get(&key).copied()) {
            remap.push(index);
            continue;
        }
        let Ok(index) = u32::try_from(retained.len()) else {
            return;
        };
        retained.push(*vertex);
        remap.push(index);
        if let Some(key) = key {
            positions.insert(key, index);
        }
    }
    // Preserve triangle order, winding, duplicate/degenerate triangles and every index entry.
    // First-source order gives deterministic vertices independent of HashMap iteration order.
    for index in &mut data.indices {
        *index = remap[*index as usize];
    }
    data.vertices = retained;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instances::{MeshInstance, MeshKind},
        mesh::{Vertex, feature_edges::EdgeGeometry},
    };
    use bytemuck::Zeroable;

    #[test]
    fn exact_remap_preserves_every_triangle_and_distinct_floats_without_merging_nonfinite_vertices()
    {
        let nan = f32::from_bits(0x7fc00005);
        let positions = [
            [-0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [f32::from_bits(1.0f32.to_bits() + 1), 0.0, 0.0],
            [nan, 0.0, 0.0],
            [nan, 0.0, 0.0],
            [f32::INFINITY, 0.0, 0.0],
            [f32::INFINITY, 0.0, 0.0],
        ];
        let mut data = MeshData {
            vertices: positions
                .into_iter()
                .map(|position| Vertex {
                    position,
                    normal: [0.0; 3],
                    uv: [0.0; 2],
                })
                .collect(),
            indices: vec![0, 1, 2, 3, 2, 1, 4, 1, 2, 5, 6, 7, 8, 8, 8],
        };
        let before = data
            .indices
            .iter()
            .map(|&i| data.vertices[i as usize].position)
            .collect::<Vec<_>>();
        deduplicate_positions(&mut data);
        assert_eq!(data.vertices.len(), 8);
        assert_eq!(data.indices.len(), before.len());
        let key = |p: [f32; 3]| p.map(|v| if v == 0.0 { 0 } else { v.to_bits() });
        for (position, &index) in before.iter().zip(&data.indices) {
            assert_eq!(key(*position), key(data.vertices[index as usize].position));
        }
        assert_eq!(data.indices[0], data.indices[3]);
        assert_ne!(data.indices[1], data.indices[6]);
        assert_ne!(
            data.indices[9], data.indices[10],
            "NaN vertices remain distinct"
        );
        assert_eq!(
            data.indices[12], data.indices[13],
            "authored degenerate triangle is retained"
        );
        let once = bytemuck::cast_slice::<_, u8>(&data.vertices).to_vec();
        let indices = data.indices.clone();
        deduplicate_positions(&mut data);
        assert_eq!(once, bytemuck::cast_slice::<_, u8>(&data.vertices));
        assert_eq!(indices, data.indices);
    }

    #[test]
    fn real_glb_depth_dedup_preserves_source_models_triangles_and_feature_edges() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/models/lamps/par-64-long-nose-silver.glb");
        let model = viz_scene::read_glb(&std::fs::read(path).unwrap()).unwrap();
        let mut scene = viz_scene::Scene::default();
        scene.models.push(model);
        let source_scene = serde_json::to_vec(&scene.models[0]).unwrap();
        let mut cache = HashMap::new();
        let mut source = 0;
        let mut unique = 0;
        let mut indices = 0;
        let drawn = (0..scene.models[0].parts.len())
            .map(|part| {
                (
                    MeshKind::ModelPart(0, part as u32),
                    vec![MeshInstance::zeroed()],
                )
            })
            .collect::<Vec<_>>();
        assert!(super::super::ensure(&mut cache, &scene, &drawn));
        for (kind, _) in &drawn {
            let original = super::super::mesh_data(&scene, *kind).unwrap();
            let depth = &cache[kind];
            source += original.vertices.len();
            unique += depth.data.vertices.len();
            indices += original.indices.len();
            assert_eq!(original.indices.len(), depth.data.indices.len());
            for (&a, &b) in original.indices.iter().zip(&depth.data.indices) {
                assert_eq!(
                    original.vertices[a as usize].position,
                    depth.data.vertices[b as usize].position
                );
            }
            let original_edges = EdgeGeometry::from_mesh(&original).edges;
            assert_eq!(original_edges.len(), depth.edges.len());
            for (a, b) in original_edges.iter().zip(&depth.edges) {
                assert_eq!(a.start.as_slice(), &b.start[..3]);
                assert_eq!(a.end.as_slice(), &b.end[..3]);
                assert_eq!(a.normal_a.as_slice(), &b.a[..3]);
                assert_eq!(a.normal_b.as_slice(), &b.b[..3]);
                assert_eq!(f32::from(a.always), b.start[3]);
            }
        }
        println!(
            "LINES_DEPTH_EXACT model=par-64-long-nose-silver parts={} source_vertices={source} unique_vertices={unique} triangle_indices={indices} reduction_percent={:.3}",
            scene.models[0].parts.len(),
            100.0 * (source - unique) as f64 / source as f64
        );
        assert!(
            unique < source,
            "real GLB must exercise exact position seam reuse"
        );
        assert_eq!(
            source_scene,
            serde_json::to_vec(&scene.models[0]).unwrap(),
            "picking/source asset is unchanged"
        );
        assert!(
            !super::super::ensure(&mut cache, &scene, &drawn),
            "clean adopted cache stays retained"
        );
    }
}
