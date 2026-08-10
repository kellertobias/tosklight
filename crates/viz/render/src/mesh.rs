//! Procedural meshes for fixture proxies, scenery, and beam volumes.
//!
//! Fixture models resolve to these proxies whenever no model asset is available, so a
//! light-producing head is always visible.

use crate::instances::MeshKind;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
    };
}

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    fn push(&mut self, position: Vec3, normal: Vec3, uv: Vec2) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            position: position.to_array(),
            normal: normal.to_array(),
            uv: uv.to_array(),
        });
        index
    }

    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3) {
        let base = self.push(corners[0], normal, Vec2::new(0.0, 0.0));
        self.push(corners[1], normal, Vec2::new(1.0, 0.0));
        self.push(corners[2], normal, Vec2::new(1.0, 1.0));
        self.push(corners[3], normal, Vec2::new(0.0, 1.0));
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// Unit cube centred on the origin, extents `-0.5..=0.5`.
pub fn unit_cube() -> MeshData {
    let mut mesh = MeshData::default();
    let h = 0.5;
    let faces: [(Vec3, [Vec3; 4]); 6] = [
        (
            Vec3::Z,
            [
                Vec3::new(-h, -h, h),
                Vec3::new(h, -h, h),
                Vec3::new(h, h, h),
                Vec3::new(-h, h, h),
            ],
        ),
        (
            Vec3::NEG_Z,
            [
                Vec3::new(h, -h, -h),
                Vec3::new(-h, -h, -h),
                Vec3::new(-h, h, -h),
                Vec3::new(h, h, -h),
            ],
        ),
        (
            Vec3::X,
            [
                Vec3::new(h, -h, h),
                Vec3::new(h, -h, -h),
                Vec3::new(h, h, -h),
                Vec3::new(h, h, h),
            ],
        ),
        (
            Vec3::NEG_X,
            [
                Vec3::new(-h, -h, -h),
                Vec3::new(-h, -h, h),
                Vec3::new(-h, h, h),
                Vec3::new(-h, h, -h),
            ],
        ),
        (
            Vec3::Y,
            [
                Vec3::new(-h, h, h),
                Vec3::new(h, h, h),
                Vec3::new(h, h, -h),
                Vec3::new(-h, h, -h),
            ],
        ),
        (
            Vec3::NEG_Y,
            [
                Vec3::new(-h, -h, -h),
                Vec3::new(h, -h, -h),
                Vec3::new(h, -h, h),
                Vec3::new(-h, -h, h),
            ],
        ),
    ];
    for (normal, corners) in faces {
        mesh.quad(corners, normal);
    }
    mesh
}

/// Unit-height cylinder along `+Y`, radius `0.5`, centred on the origin.
pub fn unit_cylinder(segments: u32) -> MeshData {
    let mut mesh = MeshData::default();
    let radius = 0.5;
    let half = 0.5;
    for segment in 0..segments {
        let a0 = segment as f32 / segments as f32 * TAU;
        let a1 = (segment + 1) as f32 / segments as f32 * TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let p0 = Vec3::new(c0 * radius, -half, s0 * radius);
        let p1 = Vec3::new(c1 * radius, -half, s1 * radius);
        let p2 = Vec3::new(c1 * radius, half, s1 * radius);
        let p3 = Vec3::new(c0 * radius, half, s0 * radius);
        let n0 = Vec3::new(c0, 0.0, s0);
        let n1 = Vec3::new(c1, 0.0, s1);
        let base = mesh.push(p0, n0, Vec2::new(0.0, 0.0));
        mesh.push(p1, n1, Vec2::new(1.0, 0.0));
        mesh.push(p2, n1, Vec2::new(1.0, 1.0));
        mesh.push(p3, n0, Vec2::new(0.0, 1.0));
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        // Caps.
        let top = mesh.push(Vec3::new(0.0, half, 0.0), Vec3::Y, Vec2::splat(0.5));
        let t0 = mesh.push(p3, Vec3::Y, Vec2::splat(0.5));
        let t1 = mesh.push(p2, Vec3::Y, Vec2::splat(0.5));
        mesh.indices.extend_from_slice(&[top, t0, t1]);
        let bottom = mesh.push(Vec3::new(0.0, -half, 0.0), Vec3::NEG_Y, Vec2::splat(0.5));
        let b0 = mesh.push(p1, Vec3::NEG_Y, Vec2::splat(0.5));
        let b1 = mesh.push(p0, Vec3::NEG_Y, Vec2::splat(0.5));
        mesh.indices.extend_from_slice(&[bottom, b0, b1]);
    }
    mesh
}

/// Unit sphere, radius `0.5`, centred on the origin.
pub fn unit_sphere(rings: u32, segments: u32) -> MeshData {
    let mut mesh = MeshData::default();
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let phi = v * std::f32::consts::PI;
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let theta = u * TAU;
            let normal = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
            mesh.push(normal * 0.5, normal, Vec2::new(u, v));
        }
    }
    let stride = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * stride + segment;
            let b = a + stride;
            mesh.indices
                .extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    mesh
}

/// Lens of diameter `1` in `XY`, centred on the origin, domed on both faces towards `±Z`.
///
/// Light leaves a lamp through a face — a lens, a lamp window, the front of a panel — and drawing
/// that face as a ball is what makes a rig read as a string of glowing marbles hung under the
/// fixtures. Scaled thin in `Z` this is what it is meant to be: a lens seated in its housing, with
/// a rim that catches the light and a curve across the glass.
///
/// Parametrised by radial fraction rather than by polar angle, so the segments are spent on the
/// rim — the silhouette an operator actually sees — instead of on the pole.
pub fn unit_lens(segments: u32, rings: u32) -> MeshData {
    let mut mesh = MeshData::default();
    let segments = segments.max(3);
    let rings = rings.max(1);
    for face in [1.0_f32, -1.0] {
        let front = face > 0.0;
        let apex = mesh.push(
            Vec3::new(0.0, 0.0, face * 0.5),
            Vec3::Z * face,
            Vec2::splat(0.5),
        );
        let mut inner: Vec<u32> = Vec::new();
        for ring in 1..=rings {
            let radial = ring as f32 / rings as f32;
            let radius = radial * 0.5;
            // The same profile a sphere has, so the rim turns away from the viewer exactly as a
            // curved piece of glass does once the instance transform squashes this flat.
            let height = face * 0.5 * (1.0 - radial * radial).max(0.0).sqrt();
            let mut outer = Vec::with_capacity(segments as usize + 1);
            for segment in 0..=segments {
                let angle = segment as f32 / segments as f32 * TAU;
                let (sin, cos) = angle.sin_cos();
                let position = Vec3::new(cos * radius, sin * radius, height);
                outer.push(mesh.push(
                    position,
                    position.normalize_or(Vec3::Z * face),
                    Vec2::new(cos * radial * 0.5 + 0.5, sin * radial * 0.5 + 0.5),
                ));
            }
            for segment in 0..segments as usize {
                let mut triangles = if inner.is_empty() {
                    vec![[apex, outer[segment], outer[segment + 1]]]
                } else {
                    vec![
                        [inner[segment], outer[segment], outer[segment + 1]],
                        [inner[segment], outer[segment + 1], inner[segment + 1]],
                    ]
                };
                for triangle in &mut triangles {
                    if !front {
                        triangle.swap(1, 2);
                    }
                    mesh.indices.extend_from_slice(triangle);
                }
            }
            inner = outer;
        }
    }
    mesh
}

/// Cone whose apex sits at the origin and whose unit-radius base sits at `+Z = 1`.
///
/// Beam passes scale this by `tan(half_angle)` in `X`/`Y` and by the beam length in `Z`, so one
/// mesh covers every beam.
pub fn unit_cone(segments: u32) -> MeshData {
    let mut mesh = MeshData::default();
    let apex = mesh.push(Vec3::ZERO, Vec3::NEG_Z, Vec2::splat(0.5));
    let mut rim = Vec::with_capacity(segments as usize + 1);
    for segment in 0..=segments {
        let angle = segment as f32 / segments as f32 * TAU;
        let (sin, cos) = angle.sin_cos();
        let position = Vec3::new(cos, sin, 1.0);
        rim.push(mesh.push(position, position.normalize(), Vec2::new(cos, sin)));
    }
    for segment in 0..segments as usize {
        mesh.indices
            .extend_from_slice(&[apex, rim[segment], rim[segment + 1]]);
    }
    let centre = mesh.push(Vec3::new(0.0, 0.0, 1.0), Vec3::Z, Vec2::splat(0.5));
    for segment in 0..segments as usize {
        mesh.indices
            .extend_from_slice(&[centre, rim[segment + 1], rim[segment]]);
    }
    mesh
}

/// Flat quad in the `XZ` plane, `1 x 1`, centred on the origin, facing `+Y`.
pub fn unit_plane() -> MeshData {
    let mut mesh = MeshData::default();
    mesh.quad(
        [
            Vec3::new(-0.5, 0.0, 0.5),
            Vec3::new(0.5, 0.0, 0.5),
            Vec3::new(0.5, 0.0, -0.5),
            Vec3::new(-0.5, 0.0, -0.5),
        ],
        Vec3::Y,
    );
    mesh
}

/// The proxy one procedural [`MeshKind`] draws, and the name it goes by.
///
/// One definition, so the geometry the device uploads and the geometry an export hands to another
/// application are the same tessellation rather than two guesses at it. A model part is not here:
/// its triangles come from the fixture package, not from this file.
pub fn procedural(kind: MeshKind) -> Option<(&'static str, MeshData)> {
    match kind {
        MeshKind::Cube => Some(("cube", unit_cube())),
        MeshKind::Cylinder => Some(("cylinder", unit_cylinder(20))),
        MeshKind::Sphere => Some(("sphere", unit_sphere(8, 14))),
        MeshKind::Lens => Some(("lens", unit_lens(24, 3))),
        MeshKind::Plane => Some(("plane", unit_plane())),
        MeshKind::ModelPart(..) | MeshKind::PlanArtwork(..) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_procedural_kind_has_geometry() {
        for kind in MeshKind::PROCEDURAL {
            let (name, mesh) = procedural(kind).expect("a procedural kind has a proxy");
            assert!(!name.is_empty());
            assert!(!mesh.vertices.is_empty(), "{name} drew nothing");
        }
        assert!(procedural(MeshKind::ModelPart(0, 0)).is_none());
        assert!(procedural(MeshKind::PlanArtwork(0)).is_none());
    }

    #[test]
    fn generated_meshes_are_non_degenerate() {
        for mesh in [
            unit_cube(),
            unit_cylinder(16),
            unit_sphere(8, 12),
            unit_lens(16, 3),
            unit_cone(24),
            unit_plane(),
        ] {
            assert!(!mesh.vertices.is_empty());
            assert!(mesh.indices.len() % 3 == 0);
            assert!(
                mesh.indices
                    .iter()
                    .all(|index| (*index as usize) < mesh.vertices.len())
            );
        }
    }
}
