use super::super::Segment;
use super::{
    CameraState, ProjectedEdge, ProjectedHeader, WireInstance, plan, projected_size,
    transforms_changed,
};
use crate::{
    instances::{MeshInstance, MeshKind},
    mesh,
};
use bytemuck::Zeroable;
use glam::{Mat3, Mat4, Quat, Vec3};

fn edges(count: usize) -> Vec<Segment> {
    vec![Segment::zeroed(); count]
}
fn instances(count: usize) -> Vec<MeshInstance> {
    vec![MeshInstance::zeroed(); count]
}

#[test]
fn wire_storage_layout_is_exact_and_ignores_material_channels() {
    assert_eq!(size_of::<WireInstance>(), 80);
    assert_eq!(size_of::<plan::Reference>(), 8);
    assert_eq!(size_of::<ProjectedEdge>(), 48);
    assert_eq!(size_of::<ProjectedHeader>(), 32);
    let a = MeshInstance::zeroed();
    let mut b = a;
    b.base_colour = [1.0; 4];
    b.emissive = [2.0; 4];
    b.surface = [3.0; 4];
    assert_eq!(
        bytemuck::bytes_of(&WireInstance::new(&a, camera())),
        bytemuck::bytes_of(&WireInstance::new(&b, camera()))
    );
}

#[test]
fn storage_boundaries_split_instances_without_duplicating_geometry_within_a_chunk() {
    let data = mesh::unit_plane();
    let edges = edges(4);
    let drawn = vec![(MeshKind::Plane, instances(5))];
    let plans = plan::build(
        &drawn,
        |_| {
            Some(plan::Asset {
                data: &data,
                edges: &edges,
            })
        },
        256,
        65535,
    )
    .unwrap();
    assert_eq!(
        plans.iter().map(|p| p.links.len()).collect::<Vec<_>>(),
        [1, 1, 1, 1, 1] // 32-byte header + 192 projected bytes per instance are tightest.
    );
    assert_eq!(
        plans
            .iter()
            .flat_map(|p| p.links.iter().copied())
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]
    );
    for p in &plans {
        assert_eq!(p.positions.len(), data.vertices.len());
        assert_eq!(p.indices.len(), p.links.len() * data.indices.len());
        assert_eq!(p.edges.len(), edges.len());
        assert_eq!(p.vertices.len(), p.links.len() * data.vertices.len());
        assert_eq!(p.edge_refs.len(), p.links.len() * edges.len());
        assert_eq!(p.edge_indices.len(), p.edge_refs.len() * 6);
        assert!(
            p.edge_indices
                .iter()
                .all(|&i| (i as usize) < p.edge_refs.len() * 4)
        );
        assert!(
            p.vertices
                .iter()
                .all(|r| (r.offset as usize) < p.positions.len()
                    && (r.instance as usize) < p.links.len())
        );
        assert!(
            p.edge_refs
                .iter()
                .all(|r| (r.offset as usize) < p.edges.len()
                    && (r.instance as usize) < p.links.len())
        );
    }
}

#[test]
fn merged_assets_rebase_indices_and_references_deterministically() {
    let data = mesh::unit_plane();
    let edges = edges(4);
    let drawn = vec![
        (MeshKind::Plane, instances(1)),
        (MeshKind::Cube, instances(1)),
    ];
    let build = || {
        plan::build(
            &drawn,
            |_| {
                Some(plan::Asset {
                    data: &data,
                    edges: &edges,
                })
            },
            4096,
            65535,
        )
        .unwrap()
    };
    let first = build();
    let second = build();
    assert_eq!(first.len(), 1);
    let p = &first[0];
    assert_eq!(p.positions.len(), data.vertices.len() * 2);
    assert_eq!(
        p.indices[data.indices.len()],
        data.indices[0] + data.vertices.len() as u32
    );
    assert_eq!(
        p.vertices[data.vertices.len()].offset,
        data.vertices.len() as u32
    );
    assert_eq!(p.edge_refs[edges.len()].offset, edges.len() as u32);
    assert_eq!(p.links, second[0].links);
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&p.vertices),
        bytemuck::cast_slice::<_, u8>(&second[0].vertices)
    );
}

#[test]
fn oversize_assets_and_invalid_indices_fail_without_truncation() {
    let mut data = mesh::unit_plane();
    let edges = edges(5);
    let drawn = vec![(MeshKind::Plane, instances(1))];
    let result = plan::build(
        &drawn,
        |_| {
            Some(plan::Asset {
                data: &data,
                edges: &edges,
            })
        },
        256,
        65535,
    );
    assert!(result.err().unwrap().to_string().contains("exceed"));
    data.indices[0] = u32::MAX;
    let result = plan::build(
        &drawn,
        |_| {
            Some(plan::Asset {
                data: &data,
                edges: &edges,
            })
        },
        4096,
        65535,
    );
    assert!(result.err().unwrap().to_string().contains("out-of-range"));
}

#[test]
fn indexed_depth_reuses_triangle_corners_without_aliasing_instance_transforms() {
    let data = mesh::unit_plane();
    let edges = edges(4);
    let drawn = vec![(MeshKind::Plane, instances(2))];
    let plans = plan::build(
        &drawn,
        |_| {
            Some(plan::Asset {
                data: &data,
                edges: &edges,
            })
        },
        4096,
        65535,
    )
    .unwrap();
    let p = &plans[0];
    assert_eq!(p.positions.len(), 4, "actual positions stored only once");
    assert_eq!(
        p.vertices.len(),
        8,
        "four transformed virtual vertices per instance"
    );
    assert_eq!(
        p.indices.len(),
        12,
        "two complete quads, with every authored triangle"
    );
    assert_eq!(&p.indices[..6], &data.indices);
    assert_eq!(
        &p.indices[6..],
        &data.indices.iter().map(|i| i + 4).collect::<Vec<_>>()
    );
    for virtual_index in 0..8 {
        let reference = &p.vertices[virtual_index];
        assert_eq!(reference.offset, virtual_index as u32 % 4);
        assert_eq!(reference.instance, virtual_index as u32 / 4);
    }
    // Resolve every indexed corner exactly as the vertex shader does: identical source geometry,
    // but the two draws' old instance IDs survive merging into one indexed draw.
    for (corner, &virtual_index) in p.indices.iter().enumerate() {
        let reference = &p.vertices[virtual_index as usize];
        assert_eq!(reference.offset, data.indices[corner % 6]);
        assert_eq!(reference.instance, (corner / 6) as u32);
    }
}

#[test]
fn indexed_aa_quad_corners_match_original_six_vertices_for_every_edge() {
    // Former shader semantics: (choose end point, signed screen-normal side).
    let original = [
        (false, -1),
        (false, 1),
        (true, 1),
        (false, -1),
        (true, 1),
        (true, -1),
    ];
    for edge in [0, 1, 1000] {
        let indices = plan::quad_indices(edge).unwrap();
        for (corner_index, virtual_vertex) in indices.into_iter().enumerate() {
            assert_eq!(virtual_vertex as usize / 4, edge);
            let corner = virtual_vertex % 4;
            let canonical = (corner >= 2, if corner == 1 || corner == 2 { 1 } else { -1 });
            assert_eq!(
                canonical, original[corner_index],
                "triangle coverage and AA coordinates preserved"
            );
        }
        assert_eq!(
            indices[0], indices[3],
            "shared first corner can reuse vertex result"
        );
        assert_eq!(
            indices[2], indices[4],
            "shared third corner can reuse vertex result"
        );
    }
    assert!(plan::quad_indices(u32::MAX as usize / 4 + 1).is_err());
}

#[test]
fn projected_output_budget_splits_chunks_without_losing_aa_indices() {
    let data = mesh::MeshData::default();
    let edges = edges(32);
    let drawn = vec![(MeshKind::Plane, instances(3))];
    let p = plan::build(
        &drawn,
        |_| {
            Some(plan::Asset {
                data: &data,
                edges: &edges,
            })
        },
        2048,
        65535,
    )
    .unwrap();
    assert_eq!(
        p.iter().map(|chunk| chunk.links.len()).collect::<Vec<_>>(),
        [1, 1, 1]
    );
    for chunk in p {
        assert!(projected_size(chunk.edge_refs.len()).unwrap() <= 2048);
        assert_eq!(
            chunk.edge_indices.len(),
            chunk.links.len() * edges.len() * 6
        );
        for (edge, indices) in chunk.edge_indices.chunks_exact(6).enumerate() {
            assert_eq!(indices, &plan::quad_indices(edge).unwrap());
        }
    }
}

#[test]
fn projected_header_is_indirect_compatible_and_only_counter_reset_preserves_draw_constants() {
    let mut header = ProjectedHeader::empty();
    header.index_count = 42;
    let words: &[u32] = bytemuck::cast_slice(std::slice::from_ref(&header));
    assert_eq!(words, &[42, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(std::mem::offset_of!(ProjectedHeader, index_count), 0);
    assert_eq!(std::mem::offset_of!(ProjectedHeader, instance_count), 4);
    assert_eq!(std::mem::offset_of!(ProjectedHeader, first_index), 8);
    assert_eq!(std::mem::offset_of!(ProjectedHeader, base_vertex), 12);
    assert_eq!(std::mem::offset_of!(ProjectedHeader, first_instance), 16);
    // This is the exact four-byte GPU clear before dirty projection, with no reset on clean
    // frames. Constants and the reserved draw count survive every read-only frame unchanged.
    let retained = header;
    assert_eq!(retained.index_count, 42);
    bytemuck::bytes_of_mut(&mut header)[..4].fill(0);
    let words: &[u32] = bytemuck::cast_slice(std::slice::from_ref(&header));
    assert_eq!(words, &[0, 1, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn projected_allocation_includes_header_runtime_record_and_atomic_counter_overflow_bound() {
    assert_eq!(projected_size(0), Some(80));
    assert_eq!(projected_size(1), Some(80));
    assert_eq!(projected_size(2), Some(128));
    let max_edges = u32::MAX as usize / 6;
    assert!(projected_size(max_edges).is_some());
    assert!(projected_size(max_edges + 1).is_none());
    assert!(projected_size(usize::MAX).is_none());
    let data = mesh::MeshData::default();
    let edges = edges(4);
    let drawn = vec![(MeshKind::Plane, instances(2))];
    let build = |limit| {
        plan::build(
            &drawn,
            |_| {
                Some(plan::Asset {
                    data: &data,
                    edges: &edges,
                })
            },
            limit,
            65535,
        )
        .unwrap()
    };
    assert_eq!(
        build(384).len(),
        2,
        "old 48-byte-only budget misses the indirect header"
    );
    let exact = build(416);
    assert_eq!(exact.len(), 1);
    assert_eq!(projected_size(exact[0].edge_refs.len()), Some(416));
    assert_eq!(
        exact[0].edge_indices.len(),
        48,
        "all eight source edges retain capacity"
    );
    // The compacted ordinal addresses the corresponding canonical prefix, independent of
    // which source edge won the atomic reservation or how many other edges were culled.
    for (ordinal, quad) in exact[0].edge_indices.chunks_exact(6).take(3).enumerate() {
        assert_eq!(quad, &plan::quad_indices(ordinal).unwrap());
    }
}

#[test]
fn dispatch_limit_splits_chunks_without_dropping_a_single_edge() {
    let data = mesh::unit_plane();
    let edges = edges(4);
    let drawn = vec![(MeshKind::Plane, instances(17))];
    let chunks = plan::build(
        &drawn,
        |_| {
            Some(plan::Asset {
                data: &data,
                edges: &edges,
            })
        },
        65536,
        1,
    )
    .unwrap();
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.links.len())
            .collect::<Vec<_>>(),
        [16, 1]
    );
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.edge_refs.len())
            .sum::<usize>(),
        17 * 4
    );
    for chunk in chunks {
        assert!(
            chunk
                .edge_refs
                .len()
                .div_ceil(super::PROJECTION_WORKGROUP as usize)
                <= 1
        );
        assert_eq!(chunk.edge_indices.len(), chunk.edge_refs.len() * 6);
    }
    assert!(
        plan::build(
            &drawn,
            |_| Some(plan::Asset {
                data: &data,
                edges: &edges
            }),
            65536,
            0
        )
        .is_err()
    );
}

fn camera() -> CameraState {
    let eye = Vec3::new(7.0, 5.0, 11.0);
    CameraState {
        clip_from_world: Mat4::perspective_rh(0.8, 1.7, 0.1, 100.0)
            * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
        eye_world: eye,
        viewport: [640, 360],
    }
}

#[test]
fn clip_and_local_facing_preserve_nonuniform_and_mirrored_affine_models() {
    let camera = camera();
    let point = Vec3::new(0.7, -0.2, 0.4);
    let midpoint = Vec3::new(-0.3, 0.8, 0.1);
    let normals = [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(0.3, -0.6, 0.8)];
    for scale in [
        Vec3::ONE,
        Vec3::new(0.2, 3.0, 1.4),
        Vec3::new(-0.2, 3.0, 1.4),
        Vec3::new(-0.2, -3.0, 1.4),
    ] {
        let model = Mat4::from_scale_rotation_translation(
            scale,
            Quat::from_rotation_y(0.73) * Quat::from_rotation_x(-0.31),
            Vec3::new(2.0, -0.5, 1.0),
        );
        let mut source = MeshInstance::zeroed();
        source.model = model.to_cols_array_2d();
        let record = WireInstance::new(&source, camera);
        assert_eq!(record.local_eye[3], 1.0);
        let old_clip = camera.clip_from_world * (model * point.extend(1.0));
        let new_clip = Mat4::from_cols_array_2d(&record.clip_from_local) * point.extend(1.0);
        assert!((old_clip - new_clip).length() < 0.00003 * old_clip.length().max(1.0));
        let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
        let world_direction = camera.eye_world - model.transform_point3(midpoint);
        let local_direction =
            Vec3::from_array(record.local_eye[..3].try_into().unwrap()) - midpoint;
        for normal in normals {
            let old_facing = (normal_matrix * normal).dot(world_direction);
            let new_facing = normal.dot(local_direction);
            assert!((old_facing - new_facing).abs() < 0.0001 * old_facing.abs().max(1.0));
            assert_eq!(old_facing.is_sign_positive(), new_facing.is_sign_positive());
        }
    }
}

#[test]
fn tiny_finite_scales_remain_classifiable_and_invalid_inverses_keep_candidates() {
    let mut source = MeshInstance::zeroed();
    source.model = Mat4::from_scale(Vec3::splat(0.00000001)).to_cols_array_2d();
    let tiny = WireInstance::new(&source, camera());
    assert_eq!(
        tiny.local_eye[3], 1.0,
        "no determinant epsilon discards tiny models"
    );
    for scale in [Vec3::new(0.0, 1.0, 1.0), Vec3::new(1e-39, 1.0, 1.0)] {
        source.model = Mat4::from_scale(scale).to_cols_array_2d();
        let record = WireInstance::new(&source, camera());
        assert_eq!(record.local_eye, [0.0; 4]);
        assert!(
            record.local_eye[3] < 0.5,
            "shader conservatively keeps silhouette candidates"
        );
        assert!(Mat4::from_cols_array_2d(&record.clip_from_local).is_finite());
    }
}

#[test]
fn camera_only_changes_refresh_records_without_rebuilding_geometry() {
    let initial = camera();
    assert!(transforms_changed(None, initial, false));
    assert!(!transforms_changed(Some(initial), initial, false));
    assert!(transforms_changed(Some(initial), initial, true));
    let mut source = MeshInstance::zeroed();
    source.model = Mat4::IDENTITY.to_cols_array_2d();
    let before = WireInstance::new(&source, initial);
    let mut eye_changed = initial;
    eye_changed.eye_world.x += 1.0;
    let mut projection_changed = initial;
    projection_changed.clip_from_world =
        Mat4::from_scale(Vec3::new(0.8, 1.0, 1.0)) * initial.clip_from_world;
    for next in [eye_changed, projection_changed] {
        assert!(
            transforms_changed(Some(initial), next, false),
            "TechnicalLinesInstances revision may stay unchanged during camera motion"
        );
        let after = WireInstance::new(&source, next);
        assert_ne!(bytemuck::bytes_of(&before), bytemuck::bytes_of(&after));
        assert!(!transforms_changed(Some(next), next, false));
    }
}

#[test]
fn viewport_only_changes_invalidate_projection_even_when_clip_records_are_unchanged() {
    let before = camera();
    let mut after = before;
    after.viewport = [1280, 720];
    assert!(transforms_changed(Some(before), after, false));
    let mut source = MeshInstance::zeroed();
    source.model = Mat4::IDENTITY.to_cols_array_2d();
    assert_eq!(
        bytemuck::bytes_of(&WireInstance::new(&source, before)),
        bytemuck::bytes_of(&WireInstance::new(&source, after))
    );
}
