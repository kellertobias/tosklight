//! The Visualizer's plan and elevation artwork for a fixture drawn with a shipped default model,
//! taken from that model's editable line drawings in `assets/models/2d`.
//!
//! The drawings are embedded by `build.rs`. The top drawing is the plan; the front drawing is the
//! front elevation and, mirrored, the back; the side drawing is the left elevation and, mirrored,
//! the right. Mirroring a page and seeing it from the other side cancel out, so a mirrored view
//! shares the unmirrored view's local geometry and differs only in which way it faces.

mod svg;

include!(concat!(env!("OUT_DIR"), "/plan_drawings.rs"));

use glam::{Vec2, Vec3};
use viz_scene::{PlanArtwork, ProjectionView};

/// Which renderer views each drawing serves.
const DRAWING_VIEWS: [(&str, &[ProjectionView]); 3] = [
    ("top", &[ProjectionView::Top]),
    ("front", &[ProjectionView::Front, ProjectionView::Back]),
    ("side", &[ProjectionView::Left, ProjectionView::Right]),
];

/// Whether the build embedded a drawing of `model` from `view` (`top`, `front` or `side`).
pub fn has_model_drawing(model: &str, view: &str) -> bool {
    embedded(model, view).is_some()
}

fn embedded(model: &str, view: &str) -> Option<&'static [u8]> {
    PLAN_DRAWINGS
        .iter()
        .find(|(id, candidate, _)| *id == model && *candidate == view)
        .map(|(_, _, bytes)| *bytes)
}

/// Plan artwork for every view `model`'s drawings cover, with its millimetres multiplied by
/// `scale` — the same scale the renderer fits the 3D body to the fixture with. A view the model
/// has no drawing for is absent, for the caller to fill some other way.
pub(crate) fn model_drawing_artwork(model: &str, scale: f32) -> Vec<PlanArtwork> {
    let mut artwork = Vec::new();
    for (name, views) in DRAWING_VIEWS {
        let Some(drawing) = embedded(model, name).and_then(svg::decode) else {
            continue;
        };
        let triangles = triangulate(&drawing.silhouette);
        let segments = segments(&drawing.lines);
        if triangles.is_empty() && segments.is_empty() {
            continue;
        }
        let metres = scale / 1000.0;
        for view in views {
            artwork.push(build(*view, &triangles, &segments, metres));
        }
    }
    artwork
}

fn build(
    view: ProjectionView,
    triangles: &[[Vec2; 3]],
    segments: &[[Vec2; 2]],
    metres: f32,
) -> PlanArtwork {
    let place = |point: Vec2| {
        let point = point * metres;
        match view {
            ProjectionView::Top => Vec3::new(point.x, 0.0, point.y),
            ProjectionView::Front | ProjectionView::Back => Vec3::new(point.x, -point.y, 0.0),
            ProjectionView::Left | ProjectionView::Right => Vec3::new(0.0, -point.y, point.x),
        }
    };
    let mut artwork = PlanArtwork {
        view,
        ..PlanArtwork::default()
    };
    let normal = artwork.facing();
    for triangle in triangles {
        let mut local = triangle.map(place);
        if (local[1] - local[0]).cross(local[2] - local[0]).dot(normal) < 0.0 {
            local.swap(1, 2);
        }
        let base = artwork.vertices.len() as u32;
        for point in local {
            artwork.vertices.push(point.to_array());
            artwork.normals.push(normal.to_array());
        }
        artwork
            .indices
            .extend_from_slice(&[base, base + 1, base + 2]);
    }
    artwork.lines = segments
        .iter()
        .flat_map(|segment| segment.map(|point| place(point).to_array()))
        .collect();
    artwork
}

/// Every visible edge as one segment, with the segments too short to see dropped.
fn segments(lines: &[Vec<svg::Point>]) -> Vec<[Vec2; 2]> {
    lines
        .iter()
        .flat_map(|line| line.windows(2))
        .map(|pair| [Vec2::from(pair[0]), Vec2::from(pair[1])])
        .filter(|[from, to]| from.distance_squared(*to) > 1e-4)
        .collect()
}

/// Fill even-odd loops with triangles by sweeping across the page.
///
/// Between two neighbouring vertex heights no edge starts or ends, so the edges crossing that
/// band cut it into spans, and even-odd filling keeps every other span. Each kept span is a
/// trapezoid — two triangles. Holes and nested islands need no special case.
fn triangulate(loops: &[Vec<svg::Point>]) -> Vec<[Vec2; 3]> {
    let mut edges = Vec::new();
    let mut heights = Vec::new();
    for points in loops {
        let points = points.iter().copied().map(Vec2::from).collect::<Vec<_>>();
        for (index, from) in points.iter().enumerate() {
            let to = points[(index + 1) % points.len()];
            heights.push(from.y);
            if (to.y - from.y).abs() > f32::EPSILON {
                edges.push(if from.y < to.y {
                    (*from, to)
                } else {
                    (to, *from)
                });
            }
        }
    }
    heights.sort_by(f32::total_cmp);
    heights.dedup_by(|next, previous| (*next - *previous).abs() < 1e-3);
    let x_at = |(low, high): (Vec2, Vec2), y: f32| {
        low.x + (high.x - low.x) * ((y - low.y) / (high.y - low.y)).clamp(0.0, 1.0)
    };
    let mut triangles = Vec::new();
    for band in heights.windows(2) {
        let (top, bottom) = (band[0], band[1]);
        let middle = (top + bottom) * 0.5;
        let mut crossings = edges
            .iter()
            .filter(|(low, high)| low.y <= middle && high.y >= middle)
            .map(|edge| (x_at(*edge, middle), x_at(*edge, top), x_at(*edge, bottom)))
            .collect::<Vec<_>>();
        crossings.sort_by(|left, right| left.0.total_cmp(&right.0));
        for [left, right] in crossings.as_chunks::<2>().0.iter().copied() {
            let corners = [
                Vec2::new(left.1, top),
                Vec2::new(right.1, top),
                Vec2::new(right.2, bottom),
                Vec2::new(left.2, bottom),
            ];
            triangles.push([corners[0], corners[1], corners[2]]);
            triangles.push([corners[0], corners[2], corners[3]]);
        }
    }
    triangles
}

#[cfg(test)]
mod tests;
