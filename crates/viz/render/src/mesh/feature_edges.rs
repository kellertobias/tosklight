//! Welded model topology for feature outlines; camera silhouettes are selected at draw time.

use super::MeshData;
use glam::DVec3;
use std::collections::BTreeMap;

/// Local-space face normals must use the instance inverse-transpose when drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureEdge {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub normal_a: [f32; 3],
    pub normal_b: [f32; 3],
    /// Boundaries, creases of at least 30 degrees, and nonmanifold edges are unconditional.
    /// Otherwise draw only when the adjacent geometric faces straddle the view direction.
    pub always: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EdgeGeometry {
    pub edges: Vec<FeatureEdge>,
}

type WeldKey = [i64; 3];
#[derive(Default)]
struct Adjacency {
    normals: [Option<DVec3>; 2],
    faces: usize,
}
impl Adjacency {
    fn add(&mut self, normal: DVec3) {
        if self.faces < 2 {
            self.normals[self.faces] = Some(normal);
        }
        self.faces += 1;
    }
}

impl EdgeGeometry {
    /// UV/normal splits do not split topology. Quantization is one millionth of mesh extent.
    /// Storage is bounded by input vertices and triangle edges; no camera-dependent cache here.
    pub fn from_mesh(mesh: &MeshData) -> Self {
        let Some((origin, extent)) = finite_bounds(mesh) else {
            return Self::default();
        };
        if extent <= 0.0 {
            return Self::default();
        }
        let epsilon = extent * 1e-6;
        let mut positions = BTreeMap::<WeldKey, DVec3>::new();
        let keys: Vec<_> = mesh
            .vertices
            .iter()
            .map(|vertex| {
                let position = DVec3::from_array(vertex.position.map(f64::from));
                if !position.is_finite() {
                    return None;
                }
                let key = ((position - origin) / epsilon)
                    .round()
                    .to_array()
                    .map(|value| value as i64);
                positions.entry(key).or_insert(position);
                Some(key)
            })
            .collect();
        let mut adjacency = BTreeMap::<(WeldKey, WeldKey), Adjacency>::new();
        for triangle in mesh.indices.chunks_exact(3) {
            let Some([a, b, c]) = triangle_keys(triangle, &keys) else {
                continue;
            };
            if a == b || b == c || c == a {
                continue;
            }
            let cross = (positions[&b] - positions[&a]).cross(positions[&c] - positions[&a]);
            let area = cross.length();
            if !area.is_finite() || area <= epsilon * epsilon {
                continue;
            }
            let normal = cross / area;
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let key = if a < b { (a, b) } else { (b, a) };
                adjacency.entry(key).or_default().add(normal);
            }
        }
        let edges = adjacency
            .into_iter()
            .filter_map(|((a, b), adjacent)| {
                let first = adjacent.normals[0]?;
                let second = adjacent.normals[1].unwrap_or(first);
                let dot = first.dot(second).clamp(-1.0, 1.0);
                // Suppress only consistently wound coplanar pairs. Reversed/coincident faces and
                // nonmanifold topology remain conservative visible features rather than vanishing.
                if adjacent.faces == 2 && dot >= 1.0 - 1e-6 {
                    return None;
                }
                Some(FeatureEdge {
                    start: positions[&a].as_vec3().to_array(),
                    end: positions[&b].as_vec3().to_array(),
                    normal_a: first.as_vec3().to_array(),
                    normal_b: second.as_vec3().to_array(),
                    always: adjacent.faces != 2 || dot <= std::f64::consts::FRAC_PI_6.cos(),
                })
            })
            .collect();
        Self { edges }
    }
}

fn triangle_keys(triangle: &[u32], keys: &[Option<WeldKey>]) -> Option<[WeldKey; 3]> {
    Some([
        *keys.get(triangle[0] as usize)?.as_ref()?,
        *keys.get(triangle[1] as usize)?.as_ref()?,
        *keys.get(triangle[2] as usize)?.as_ref()?,
    ])
}

fn finite_bounds(mesh: &MeshData) -> Option<(DVec3, f64)> {
    let mut points = mesh
        .vertices
        .iter()
        .map(|vertex| DVec3::from_array(vertex.position.map(f64::from)))
        .filter(|point| point.is_finite());
    let first = points.next()?;
    let (mut min, mut max) = (first, first);
    for point in points {
        min = min.min(point);
        max = max.max(point);
    }
    Some((min, (max - min).max_element()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{Vertex, unit_cube, unit_cylinder, unit_plane};

    #[test]
    fn cube_has_twelve_creases_without_face_diagonals_despite_vertex_seams() {
        let edges = EdgeGeometry::from_mesh(&unit_cube()).edges;
        assert_eq!(edges.len(), 12);
        assert!(edges.iter().all(|edge| edge.always));
        assert!(edges.iter().all(|edge| {
            (DVec3::from_array(edge.end.map(f64::from))
                - DVec3::from_array(edge.start.map(f64::from)))
            .length()
                < 1.001
        }));
    }

    #[test]
    fn closed_cylinder_keeps_smooth_silhouette_candidates_and_cap_creases() {
        let edges = EdgeGeometry::from_mesh(&unit_cylinder(32)).edges;
        assert_eq!(edges.iter().filter(|edge| edge.always).count(), 64);
        assert_eq!(edges.iter().filter(|edge| !edge.always).count(), 32);
    }

    #[test]
    fn open_plane_keeps_four_boundaries_and_ignores_shading_attributes() {
        let mut plane = unit_plane();
        for vertex in &mut plane.vertices {
            vertex.normal = [f32::NAN; 3];
            vertex.uv = [f32::NAN; 2];
        }
        let edges = EdgeGeometry::from_mesh(&plane).edges;
        assert_eq!(edges.len(), 4);
        assert!(edges.iter().all(|edge| edge.always));
    }

    #[test]
    fn bad_indices_nonfinite_and_collapsed_triangles_do_not_drop_valid_geometry() {
        let mut cube = unit_cube();
        let invalid = cube.vertices.len() as u32;
        cube.vertices.push(Vertex {
            position: [f32::NAN, 0.0, 0.0],
            normal: [0.0; 3],
            uv: [0.0; 2],
        });
        cube.indices
            .extend_from_slice(&[0, 0, 0, invalid, 0, 1, u32::MAX, 0, 1, 1]);
        assert_eq!(EdgeGeometry::from_mesh(&cube).edges.len(), 12);
        assert!(
            EdgeGeometry::from_mesh(&MeshData::default())
                .edges
                .is_empty()
        );
    }

    #[test]
    fn opposite_winding_and_nonmanifold_edges_remain_conservative() {
        let mut plane = unit_plane();
        let [a, b, c] = [plane.indices[0], plane.indices[1], plane.indices[2]];
        plane.indices.extend_from_slice(&[c, b, a]);
        let edges = EdgeGeometry::from_mesh(&plane).edges;
        assert!(edges.len() >= 4);
        assert!(edges.iter().all(|edge| edge.always));
    }

    #[test]
    fn extraction_is_deterministic_and_invariant_under_uniform_scale() {
        let mut cube = unit_cube();
        let before = EdgeGeometry::from_mesh(&cube);
        assert_eq!(before.edges, EdgeGeometry::from_mesh(&cube).edges);
        for vertex in &mut cube.vertices {
            vertex.position = vertex.position.map(|value| value * 1e-9);
        }
        let after = EdgeGeometry::from_mesh(&cube);
        assert_eq!(before.edges.len(), after.edges.len());
        assert!(after.edges.iter().all(|edge| edge.always));
    }

    #[test]
    fn real_fixture_glb_retains_model_derived_edges_without_gpu() {
        let model = viz_scene::read_glb(include_bytes!(
            "../../../../../assets/models/lamps/led-par-pizza.glb"
        ))
        .unwrap();
        assert!(!model.parts.is_empty());
        let mut total = 0;
        for part in &model.parts {
            let mesh = MeshData {
                vertices: part
                    .positions
                    .iter()
                    .map(|position| Vertex {
                        position: *position,
                        normal: [0.0; 3],
                        uv: [0.0; 2],
                    })
                    .collect(),
                indices: part.indices.clone(),
            };
            let geometry = EdgeGeometry::from_mesh(&mesh);
            assert!(geometry.edges.len() <= part.indices.len());
            assert!(geometry.edges.iter().all(|edge| {
                edge.start
                    .iter()
                    .chain(edge.end.iter())
                    .all(|value| value.is_finite())
            }));
            total += geometry.edges.len();
        }
        assert!(total > 0);
    }
}
