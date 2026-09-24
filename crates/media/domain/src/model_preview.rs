//! A still picture of a 3D model, for choosing one by its shape.
//!
//! The model is drawn from the mapping camera's own direction, turned a little so its depth shows:
//! 30° of pan and 20° of tilt, which looks at the front from above and to the left. Its surface
//! carries a test card by texture coordinates — a checker, with the image's top-left quarter warm
//! — so the picture tells both the shape and which way a layer's image lands on it. Faces are lit
//! from the upper left, so a flat shape reads flat and a round one round.
//!
//! This is a plain CPU rasteriser over the same geometry the outputs draw: no GPU, no files, and
//! the same answer on every machine, which is what a thumbnail served to a browser needs.

use crate::model_library::ModelGeometry;
use crate::model_projection::{Matrix4, camera_distance, model_rotation, transform};

/// The turn a preview is drawn at, in the layer's own pan and tilt.
pub const PREVIEW_PAN_DEGREES: f32 = 30.0;
pub const PREVIEW_TILT_DEGREES: f32 = -20.0;

/// Pixels drawn per preview pixel along each axis, averaged for smooth edges.
const SUPERSAMPLE: usize = 2;
/// How much of the picture's height the model's bounding sphere spans.
const FILL: f32 = 0.86;

/// The model drawn into a square of `size` pixels as straight RGBA, transparent around it.
pub fn render_model_preview(geometry: &ModelGeometry, size: usize) -> Vec<u8> {
    let size = size.max(1);
    let wide = size * SUPERSAMPLE;
    let mut colour = vec![[0.0_f32; 4]; wide * wide];
    let mut depth = vec![f32::INFINITY; wide * wide];
    let rotation = model_rotation(PREVIEW_PAN_DEGREES, PREVIEW_TILT_DEGREES, 0.0);
    let (centre, radius) = bounds(geometry);
    let focal = camera_distance();
    let scale = FILL / radius.max(f32::EPSILON);

    let project = |position: [f32; 3]| -> ([f32; 3], [f32; 3]) {
        let local = [
            (position[0] - centre[0]) * scale,
            (position[1] - centre[1]) * scale,
            (position[2] - centre[2]) * scale,
        ];
        let turned = transform(&rotation, local);
        let distance = focal - turned[2];
        let half = wide as f32 / 2.0;
        let screen = [
            half + turned[0] * focal / distance * half,
            half - turned[1] * focal / distance * half,
            distance,
        ];
        (screen, [turned[0], turned[1], turned[2]])
    };

    let projected: Vec<([f32; 3], [f32; 3])> = geometry
        .vertices
        .iter()
        .map(|vertex| project(vertex.position))
        .collect();
    for triangle in geometry.indices.chunks_exact(3) {
        let corners = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        if corners.iter().any(|&index| index >= projected.len()) {
            continue;
        }
        let normals = corners.map(|index| turn(&rotation, geometry.vertices[index].normal));
        let uvs = corners.map(|index| geometry.vertices[index].uv);
        let points = corners.map(|index| projected[index].0);
        fill_triangle(&points, &normals, &uvs, wide, &mut colour, &mut depth);
    }
    downsample(&colour, wide, size)
}

/// The centre of the model's box and the radius of the sphere around it.
fn bounds(geometry: &ModelGeometry) -> ([f32; 3], f32) {
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for vertex in &geometry.vertices {
        for axis in 0..3 {
            low[axis] = low[axis].min(vertex.position[axis]);
            high[axis] = high[axis].max(vertex.position[axis]);
        }
    }
    if !low[0].is_finite() {
        return ([0.0; 3], 1.0);
    }
    let centre = [0, 1, 2].map(|axis| (low[axis] + high[axis]) / 2.0);
    let radius = geometry
        .vertices
        .iter()
        .map(|vertex| {
            let offset = [0, 1, 2].map(|axis| vertex.position[axis] - centre[axis]);
            (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]).sqrt()
        })
        .fold(0.0_f32, f32::max);
    (centre, radius)
}

/// A normal turned with the model; a rotation has no translation, so turning it as a point is
/// the same.
fn turn(rotation: &Matrix4, normal: [f32; 3]) -> [f32; 3] {
    let turned = transform(rotation, normal);
    [turned[0], turned[1], turned[2]]
}

fn fill_triangle(
    points: &[[f32; 3]; 3],
    normals: &[[f32; 3]; 3],
    uvs: &[[f32; 2]; 3],
    wide: usize,
    colour: &mut [[f32; 4]],
    depth: &mut [f32],
) {
    let [a, b, c] = points;
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < f32::EPSILON {
        return;
    }
    let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0) as usize;
    let max_x = (a[0].max(b[0]).max(c[0]).ceil().max(0.0) as usize).min(wide - 1);
    let min_y = a[1].min(b[1]).min(c[1]).floor().max(0.0) as usize;
    let max_y = (a[1].max(b[1]).max(c[1]).ceil().max(0.0) as usize).min(wide - 1);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
            let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let at = y * wide + x;
            let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
            if z >= depth[at] {
                continue;
            }
            depth[at] = z;
            let blend = |values: [[f32; 3]; 3]| {
                [0, 1, 2]
                    .map(|axis| w0 * values[0][axis] + w1 * values[1][axis] + w2 * values[2][axis])
            };
            let normal = blend(*normals);
            let uv = [0, 1].map(|axis| w0 * uvs[0][axis] + w1 * uvs[1][axis] + w2 * uvs[2][axis]);
            colour[at] = shade(normal, uv);
        }
    }
}

/// The test card at `uv`, lit from the upper left and a little in front.
fn shade(normal: [f32; 3], uv: [f32; 2]) -> [f32; 4] {
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
        .sqrt()
        .max(f32::EPSILON);
    let light = [-0.45_f32, 0.55, 0.70];
    // A face seen from behind is lit as its front would be, only dimmer.
    let facing = (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2]) / length;
    let lit = 0.45 + 0.55 * facing.abs() * if normal[2] < 0.0 { 0.6 } else { 1.0 };
    let (u, v) = (uv[0].clamp(0.0, 1.0), uv[1].clamp(0.0, 1.0));
    let checker = ((u * 8.0).floor() as i32 + (v * 8.0).floor() as i32) % 2 == 0;
    let base = if u < 0.5 && v < 0.5 {
        if checker {
            [0.98, 0.62, 0.22]
        } else {
            [0.86, 0.44, 0.14]
        }
    } else if checker {
        [0.42, 0.82, 0.90]
    } else {
        [0.20, 0.52, 0.70]
    };
    [base[0] * lit, base[1] * lit, base[2] * lit, 1.0]
}

/// Averages each block of supersamples into one straight-alpha RGBA pixel.
fn downsample(colour: &[[f32; 4]], wide: usize, size: usize) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(size * size * 4);
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0.0_f32; 4];
            for dy in 0..SUPERSAMPLE {
                for dx in 0..SUPERSAMPLE {
                    let sample = colour[(y * SUPERSAMPLE + dy) * wide + x * SUPERSAMPLE + dx];
                    for channel in 0..3 {
                        sum[channel] += sample[channel] * sample[3];
                    }
                    sum[3] += sample[3];
                }
            }
            let alpha = sum[3] / samples;
            for channel in 0..3 {
                let straight = if sum[3] > 0.0 {
                    sum[channel] / sum[3]
                } else {
                    0.0
                };
                pixels.push((straight.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
            pixels.push((alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuiltinModel;

    const SIZE: usize = 64;

    fn covered(pixels: &[u8]) -> Vec<bool> {
        pixels.chunks_exact(4).map(|pixel| pixel[3] > 127).collect()
    }

    fn coverage(pixels: &[u8]) -> usize {
        covered(pixels).into_iter().filter(|&inside| inside).count()
    }

    #[test]
    fn every_built_in_draws_its_own_shape_inside_the_picture() {
        let pictures: Vec<Vec<bool>> = BuiltinModel::ALL
            .iter()
            .map(|model| covered(&render_model_preview(&model.geometry(), SIZE)))
            .collect();
        for (model, picture) in BuiltinModel::ALL.iter().zip(&pictures) {
            let inside = picture.iter().filter(|&&inside| inside).count();
            assert!(inside > SIZE * SIZE / 8, "{model:?} covers {inside} pixels");
            // Nothing touches the edge: the whole shape is in the picture.
            for index in 0..SIZE {
                assert!(
                    !picture[index] && !picture[SIZE * (SIZE - 1) + index],
                    "{model:?}"
                );
                assert!(
                    !picture[index * SIZE] && !picture[index * SIZE + SIZE - 1],
                    "{model:?}"
                );
            }
        }
        for first in 0..pictures.len() {
            for second in first + 1..pictures.len() {
                assert_ne!(
                    pictures[first], pictures[second],
                    "two built-ins look alike"
                );
            }
        }
    }

    #[test]
    fn the_image_top_left_shows_where_it_lands() {
        // The Plane shows the image upright facing the camera; turned 30°, its top-left quarter
        // is still up and to the left, and it is the warm one.
        let pixels = render_model_preview(&BuiltinModel::Plane.geometry(), SIZE);
        let at = |x: usize, y: usize| &pixels[(y * SIZE + x) * 4..(y * SIZE + x) * 4 + 4];
        let upper_left = at(SIZE * 3 / 8, SIZE * 3 / 8);
        let lower_right = at(SIZE * 5 / 8, SIZE * 5 / 8);
        assert_eq!(upper_left[3], 255);
        assert!(upper_left[0] > upper_left[2], "warm: {upper_left:?}");
        assert!(lower_right[2] > lower_right[0], "cool: {lower_right:?}");
    }

    #[test]
    fn the_picture_is_the_same_every_time_and_empty_geometry_is_blank() {
        let cube = BuiltinModel::Cube.geometry();
        assert_eq!(
            render_model_preview(&cube, SIZE),
            render_model_preview(&cube, SIZE)
        );
        let empty = ModelGeometry {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        assert_eq!(coverage(&render_model_preview(&empty, SIZE)), 0);
    }
}
