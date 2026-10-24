use super::super::Segment;
use super::{WireInstance, plan};
use crate::{
    instances::{MeshInstance, MeshKind},
    mesh,
};
use bytemuck::Zeroable;

fn edges(count: usize) -> Vec<Segment> {
    vec![Segment::zeroed(); count]
}
fn instances(count: usize) -> Vec<MeshInstance> {
    vec![MeshInstance::zeroed(); count]
}

#[test]
fn wire_storage_layout_is_exact_and_ignores_material_channels() {
    assert_eq!(size_of::<WireInstance>(), 112);
    assert_eq!(size_of::<plan::Reference>(), 8);
    let a = MeshInstance::zeroed();
    let mut b = a;
    b.base_colour = [1.0; 4];
    b.emissive = [2.0; 4];
    b.surface = [3.0; 4];
    assert_eq!(
        bytemuck::bytes_of(&WireInstance::from(&a)),
        bytemuck::bytes_of(&WireInstance::from(&b))
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
    )
    .unwrap();
    assert_eq!(
        plans.iter().map(|p| p.links.len()).collect::<Vec<_>>(),
        [2, 2, 1]
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
        assert_eq!(p.indices.len(), data.indices.len());
        assert_eq!(p.edges.len(), edges.len());
        assert_eq!(p.triangles.len(), p.links.len() * data.indices.len() / 3);
        assert_eq!(p.edge_refs.len(), p.links.len() * edges.len());
        assert!(
            p.triangles
                .iter()
                .all(|r| r.offset as usize + 2 < p.indices.len()
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
        p.triangles[data.indices.len() / 3].offset,
        data.indices.len() as u32
    );
    assert_eq!(p.edge_refs[edges.len()].offset, edges.len() as u32);
    assert_eq!(p.links, second[0].links);
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&p.triangles),
        bytemuck::cast_slice::<_, u8>(&second[0].triangles)
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
    );
    assert!(result.err().unwrap().to_string().contains("out-of-range"));
}
