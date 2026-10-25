//! A true constant-radius shaft with hemispherical ends, shared at anatomical joint centres.
use super::super::{MeshData, Vertex};
use glam::Vec3;
const SIDES: usize = 8;
const HEMISPHERE_RINGS: usize = 3;
pub(super) fn build(length: f32, diameter: f32) -> MeshData {
    let radius = diameter * 0.5;
    let mut mesh = MeshData::default();
    for half in 0..2 {
        for ring in 0..=HEMISPHERE_RINGS {
            let angle =
                (half as f32 + ring as f32 / HEMISPHERE_RINGS as f32) * std::f32::consts::FRAC_PI_2;
            let centre = if half == 0 {
                length * 0.5
            } else {
                -length * 0.5
            };
            for side in 0..=SIDES {
                let azimuth = side as f32 / SIDES as f32 * std::f32::consts::TAU;
                let normal = Vec3::new(
                    angle.sin() * azimuth.cos(),
                    angle.cos(),
                    angle.sin() * azimuth.sin(),
                );
                let point = normal * radius + Vec3::new(0.0, centre, 0.0);
                mesh.vertices.push(Vertex {
                    position: point.to_array(),
                    normal: normal.to_array(),
                    uv: [
                        side as f32 / SIDES as f32,
                        (half * (HEMISPHERE_RINGS + 1) + ring) as f32 / 7.0,
                    ],
                });
            }
        }
    }
    let stride = (SIDES + 1) as u32;
    for ring in 0..(HEMISPHERE_RINGS * 2 + 1) as u32 {
        for side in 0..SIDES as u32 {
            let a = ring * stride + side;
            let b = a + stride;
            mesh.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    mesh
}
