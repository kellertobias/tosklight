//! Editable line drawings of a shipped 3D model, one orthographic view at a time, drawn the way a
//! draughtsman would: the outline, the creases and the edges between parts, with every line hidden
//! behind the model left out.
//!
//! A drawing is a starting point for a hand-finished fixture symbol, not a finished contract. Each
//! is plain SVG in millimetres with named groups — `silhouette`, `base`, `yoke`, `head` and
//! `origin` — so an editor can restyle, simplify or redraw any part of it. The views and the poses
//! a moving head is drawn in match the projections the CAD already draws, so an improved drawing can
//! take a generated one's place.

mod pose;

pub use pose::{DEFAULT_LEAN_DEGREES, DrawingPose, drawing_pose, faces_forward, lean_target};

use crate::projection::{ProjectionError, depth, head_rotation, project, simplified_away};
use glam::{Vec2, Vec3};
use light_fixture::ProfileProjectionView;
use std::collections::HashMap;
use std::fmt::Write as _;
use viz_scene::{FixtureModel, ModelPartKind};

/// The views every model is drawn in, with the name each drawing is saved under.
pub const DRAWING_VIEWS: [(&str, ProfileProjectionView); 3] = [
    ("top", ProfileProjectionView::Top),
    ("front", ProfileProjectionView::Front),
    ("side", ProfileProjectionView::Left),
];

pub const DRAWING_GENERATOR_ID: &str = "tosklight.model-drawing";
pub const DRAWING_GENERATOR_VERSION: &str = "2";

/// Two faces meeting at more than this angle draw a crease between them.
const CREASE_DEGREES: f32 = 35.0;
/// The finest a view is rasterised to decide what is hidden, along its longest side.
const RASTER_SIDE: f32 = 1600.0;

/// One view of a model reduced to what a drawing needs, in page millimetres with y down.
#[derive(Debug, Default)]
pub struct ModelDrawing {
    pub pose: &'static str,
    pub min: Vec2,
    pub max: Vec2,
    /// Closed loops of the area the model covers; a loop inside another is a hole.
    pub silhouette: Vec<Vec<Vec2>>,
    /// Visible edges as polylines, each with the part it belongs to.
    pub lines: Vec<(ModelPartKind, Vec<Vec2>)>,
    /// The hanging hardware's own area, when it is drawn apart from the body.
    pub hardware_silhouette: Vec<Vec<Vec2>>,
    /// The hanging hardware's visible edges, hidden only by the hardware itself: it is in front of
    /// the body wherever the two overlap, so a consumer hides body lines under it.
    pub hardware_lines: Vec<Vec<Vec2>>,
    /// Where the body turns in its hanging frame, on the page, when the hardware is drawn apart.
    pub hinge: Option<Vec2>,
    /// The bracket angle the drawn body's pose equals, in degrees about the lamp's transverse axis
    /// as the Visualizer turns a bracket, when the body is turned in its frame.
    pub bracket_degrees: Option<f32>,
}

struct Face {
    corners: [u32; 3],
    normal: Vec3,
    part: usize,
    kind: ModelPartKind,
    /// Part of the hanging frame or truss coupler rather than the lamp.
    hardware: bool,
}

/// Draw `model` from `view` as an SVG document labelled with the model's id and the view's name.
pub fn model_drawing_svg(
    model: &FixtureModel,
    label: &str,
    name: &str,
    view: ProfileProjectionView,
) -> Result<String, ProjectionError> {
    Ok(drawing_svg(&model_drawing(model, view)?, label, name))
}

/// A lamp's body turned about its bracket hinge for one drawing, so a view shows it the way it
/// reads best: a PAR seen from above pointing forward, a blinder facing the audience. The mounting
/// hardware — the hanging frame and the truss coupler — stays where it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyTilt {
    /// The point the body turns about, in model metres.
    pub hinge: Vec3,
    /// The direction the body's light leaves in once turned.
    pub target: Vec3,
}

/// Reduce `model` seen from `view` to its silhouette and its visible edges.
pub fn model_drawing(
    model: &FixtureModel,
    view: ProfileProjectionView,
) -> Result<ModelDrawing, ProjectionError> {
    model_drawing_with(model, view, None)
}

/// Draw `model` from `view` with its body turned by `tilt` as an SVG document.
pub fn model_drawing_svg_with(
    model: &FixtureModel,
    label: &str,
    name: &str,
    view: ProfileProjectionView,
    tilt: Option<BodyTilt>,
) -> Result<String, ProjectionError> {
    Ok(drawing_svg(
        &model_drawing_with(model, view, tilt)?,
        label,
        name,
    ))
}

/// Whether a part is the hardware a lamp hangs by rather than the lamp itself.
fn mounting_hardware(name: &str) -> bool {
    let folded = name.to_ascii_lowercase();
    folded.contains("hanging-frame") || folded.contains("truss-coupler")
}

/// Reduce `model` seen from `view`, with its body turned by `tilt`, to its silhouette and edges.
pub fn model_drawing_with(
    model: &FixtureModel,
    view: ProfileProjectionView,
    tilt: Option<BodyTilt>,
) -> Result<ModelDrawing, ProjectionError> {
    let pose = DrawingPose {
        hinge: tilt.map(|tilt| tilt.hinge),
        body_target: tilt.map(|tilt| tilt.target),
        ..DrawingPose::default()
    };
    model_drawing_posed(model, view, &pose)
}

/// Draw `model` from `view` in `pose` as an SVG document.
pub fn model_drawing_svg_posed(
    model: &FixtureModel,
    label: &str,
    name: &str,
    view: ProfileProjectionView,
    pose: &DrawingPose,
) -> Result<String, ProjectionError> {
    Ok(drawing_svg(
        &model_drawing_posed(model, view, pose)?,
        label,
        name,
    ))
}

/// Reduce `model` seen from `view`, posed by `pose`, to its silhouette and edges.
///
/// With a hinge, the hanging hardware is traced apart from the body: each is hidden only by
/// itself, so a consumer can turn the body about the hinge and still lay the hardware over it.
pub fn model_drawing_posed(
    model: &FixtureModel,
    view: ProfileProjectionView,
    pose: &DrawingPose,
) -> Result<ModelDrawing, ProjectionError> {
    let target = model.has_head.then_some(match (pose.head_target, view) {
        (Some(target), _) => target,
        (None, ProfileProjectionView::Top) => Vec3::Z,
        (None, _) => Vec3::NEG_Y,
    });
    let name = match target {
        None if pose.body_target.is_some() => "tilted",
        None => "authored-home",
        Some(axis) if axis == Vec3::NEG_Y => "moving-down",
        Some(_) => "moving-forward",
    };
    let rotation = head_rotation(model, target);
    let turn = pose.body_target.map(|target| {
        let from = model
            .emitter_axis
            .unwrap_or(Vec3::NEG_Y)
            .normalize_or(Vec3::NEG_Y);
        (
            pose.hinge.unwrap_or(Vec3::ZERO),
            glam::Quat::from_rotation_arc(from, target.normalize_or(Vec3::Z)),
        )
    });
    // Depth grows away from the viewer, so the viewer looks back along the negated depth axis.
    let toward = -Vec3::new(
        depth(Vec3::X, view),
        depth(Vec3::Y, view),
        depth(Vec3::Z, view),
    );

    let (vertices, faces) = gather_faces(model, view, rotation, turn);
    if faces.is_empty() {
        return Err(ProjectionError(format!(
            "{} drawing contains no visible geometry",
            view.wire()
        )));
    }

    let split = pose.hinge.is_some()
        && faces.iter().any(|face| face.hardware)
        && faces.iter().any(|face| !face.hardware);
    let (body, hardware): (Vec<Face>, Vec<Face>) = if split {
        faces.into_iter().partition(|face| !face.hardware)
    } else {
        (faces, Vec::new())
    };

    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for (point, _) in &vertices {
        min = min.min(*point);
        max = max.max(*point);
    }
    let (lines, silhouette) = trace(&body, &vertices, toward);
    let (hardware_lines, hardware_silhouette) = if hardware.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        trace(&hardware, &vertices, toward)
    };
    Ok(ModelDrawing {
        pose: name,
        min,
        max,
        silhouette,
        lines,
        hardware_silhouette,
        hardware_lines: hardware_lines.into_iter().map(|(_, line)| line).collect(),
        hinge: pose
            .hinge
            .filter(|_| split)
            .map(|hinge| project(hinge * 1000.0, view)),
        bracket_degrees: turn
            .filter(|_| split)
            .and_then(|(_, turn)| bracket_about_x(turn)),
    })
}

/// The bracket angle `turn` is, in degrees about +X (`Quat::from_rotation_x`, as the Visualizer
/// turns a bracket) within ±180°, or `None` when it turns about some other axis.
fn bracket_about_x(turn: glam::Quat) -> Option<f32> {
    let (axis, angle) = turn.to_axis_angle();
    if angle.abs() < 1e-5 {
        return Some(0.0);
    }
    (axis.x.abs() > 0.999).then(|| {
        let degrees = (angle * axis.x.signum()).to_degrees();
        if degrees > 180.0 {
            degrees - 360.0
        } else if degrees <= -180.0 {
            degrees + 360.0
        } else {
            degrees
        }
    })
}

/// The visible edges and the outline of one set of faces, hidden only by those faces. Every set
/// is rasterised over all of `vertices`, so outlines of different sets share one grid.
#[allow(clippy::type_complexity)]
fn trace(
    faces: &[Face],
    vertices: &[(Vec2, f32)],
    toward: Vec3,
) -> (Vec<(ModelPartKind, Vec<Vec2>)>, Vec<Vec<Vec2>>) {
    let raster = Raster::of(faces, vertices);
    let segments = visible_segments(faces, vertices, &raster, toward);
    let tolerance = (raster.pixel * 0.25).max(0.05);
    let lines = chain(&segments)
        .into_iter()
        .map(|(kind, line)| (kind, simplify(&line, tolerance)))
        .filter(|(_, line)| line.len() >= 2)
        .collect();
    let silhouette = raster
        .outline()
        .into_iter()
        .map(|mut ring| {
            ring.push(ring[0]);
            let mut simple = simplify(&ring, raster.pixel * 0.6);
            simple.pop();
            simple
        })
        .filter(|ring| ring.len() >= 3)
        .collect();
    (lines, silhouette)
}

/// Every face of `model` placed for `view`, over vertices welded by position so an edge a model
/// splits for shading is still one edge. Each vertex keeps its page point and depth.
fn gather_faces(
    model: &FixtureModel,
    view: ProfileProjectionView,
    rotation: glam::Quat,
    turn: Option<(Vec3, glam::Quat)>,
) -> (Vec<(Vec2, f32)>, Vec<Face>) {
    let mut vertices: Vec<(Vec2, f32)> = Vec::new();
    let mut welded: HashMap<[i64; 3], u32> = HashMap::new();
    let mut faces = Vec::new();
    for (part_index, part) in model.parts.iter().enumerate() {
        if simplified_away(&part.name) {
            continue;
        }
        let head = part.kind == ModelPartKind::Head;
        let hardware = !head && mounting_hardware(&part.name);
        let turned = turn.filter(|_| !head && !hardware);
        let place = |point: Vec3| {
            (if head {
                model.head_pivot + rotation * (point - model.head_pivot)
            } else if let Some((hinge, turn)) = turned {
                hinge + turn * (point - hinge)
            } else {
                point
            }) * 1000.0
        };
        for indices in part.indices.as_chunks::<3>().0 {
            let corners = [indices[0], indices[1], indices[2]].map(|index| index as usize);
            let Some(world) = corners
                .iter()
                .map(|index| {
                    part.positions
                        .get(*index)
                        .map(|p| place(Vec3::from_array(*p)))
                })
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let mut normal = (world[1] - world[0]).cross(world[2] - world[0]);
            if normal.length_squared() < 1e-8 {
                continue;
            }
            // A face wound the wrong way round is corrected by the normals the model carries.
            let shading = corners
                .iter()
                .filter_map(|index| part.normals.get(*index))
                .map(|normal| Vec3::from_array(*normal))
                .sum::<Vec3>();
            let shading = if head {
                rotation * shading
            } else if let Some((_, turn)) = turned {
                turn * shading
            } else {
                shading
            };
            if shading.length_squared() > 1e-10 && normal.dot(shading) < 0.0 {
                normal = -normal;
            }
            let ids = [world[0], world[1], world[2]].map(|point| {
                let key = (point * 100.0).round().as_i64vec3().to_array();
                *welded.entry(key).or_insert_with(|| {
                    vertices.push((project(point, view), depth(point, view)));
                    vertices.len() as u32 - 1
                })
            });
            if ids[0] == ids[1] || ids[1] == ids[2] || ids[2] == ids[0] {
                continue;
            }
            faces.push(Face {
                corners: ids,
                normal: normal.normalize(),
                part: part_index,
                kind: part.kind,
                hardware,
            });
        }
    }
    (vertices, faces)
}

/// The visible stretches of every edge a drawing shows: outlines where a face turns away,
/// creases, part boundaries and open edges.
fn visible_segments(
    faces: &[Face],
    vertices: &[(Vec2, f32)],
    raster: &Raster,
    toward: Vec3,
) -> Vec<(ModelPartKind, Vec2, Vec2)> {
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (index, face) in faces.iter().enumerate() {
        let [a, b, c] = face.corners;
        for (first, second) in [(a, b), (b, c), (c, a)] {
            edges
                .entry((first.min(second), first.max(second)))
                .or_default()
                .push(index);
        }
    }
    let facing = |face: &Face| face.normal.dot(toward) > 1e-4;
    let crease = CREASE_DEGREES.to_radians().cos();
    let mut segments: Vec<(ModelPartKind, Vec2, Vec2)> = Vec::new();
    let mut sorted: Vec<_> = edges.iter().collect();
    sorted.sort_by_key(|(key, _)| **key);
    for ((first, second), owners) in sorted {
        let feature = match owners.as_slice() {
            [_] => true,
            [left, right] => {
                let (left, right) = (&faces[*left], &faces[*right]);
                let (left_facing, right_facing) = (facing(left), facing(right));
                left_facing != right_facing
                    || ((left_facing || right_facing)
                        && (left.normal.dot(right.normal) < crease || left.part != right.part))
            }
            _ => true,
        };
        if !feature {
            continue;
        }
        let kind = owners
            .iter()
            .map(|index| &faces[*index])
            .find(|face| facing(face))
            .unwrap_or(&faces[owners[0]])
            .kind;
        let (start, end) = (vertices[*first as usize], vertices[*second as usize]);
        for (from, to) in raster.visible_runs(start, end) {
            segments.push((kind, from, to));
        }
    }
    segments
}

/// The nearest depth under every pixel of a view, for deciding what an edge is behind.
struct Raster {
    origin: Vec2,
    pixel: f32,
    width: usize,
    height: usize,
    nearest: Vec<f32>,
}

impl Raster {
    fn of(faces: &[Face], vertices: &[(Vec2, f32)]) -> Self {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for (point, _) in vertices {
            min = min.min(*point);
            max = max.max(*point);
        }
        let pixel = ((max - min).max_element() / RASTER_SIDE).max(0.05);
        let width = ((max.x - min.x) / pixel).ceil() as usize + 3;
        let height = ((max.y - min.y) / pixel).ceil() as usize + 3;
        let mut raster = Self {
            origin: min - Vec2::splat(pixel),
            pixel,
            width,
            height,
            nearest: vec![f32::INFINITY; width * height],
        };
        for face in faces {
            raster.fill(face.corners.map(|corner| vertices[corner as usize]));
        }
        raster
    }

    fn fill(&mut self, corners: [(Vec2, f32); 3]) {
        let [a, b, c] = corners.map(|(point, _)| (point - self.origin) / self.pixel);
        let area = (b - a).perp_dot(c - a);
        if area.abs() < 1e-9 {
            return;
        }
        let low = a.min(b).min(c).floor().max(Vec2::ZERO);
        let high = a.max(b).max(c).ceil();
        let x1 = (high.x.max(0.0) as usize).min(self.width - 1);
        let y1 = (high.y.max(0.0) as usize).min(self.height - 1);
        for y in low.y as usize..=y1 {
            for x in low.x as usize..=x1 {
                let point = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let wa = (b - point).perp_dot(c - point) / area;
                let wb = (c - point).perp_dot(a - point) / area;
                let wc = 1.0 - wa - wb;
                if wa < -1e-4 || wb < -1e-4 || wc < -1e-4 {
                    continue;
                }
                let depth = wa * corners[0].1 + wb * corners[1].1 + wc * corners[2].1;
                let cell = &mut self.nearest[y * self.width + x];
                *cell = cell.min(depth);
            }
        }
    }

    fn at(&self, point: Vec2) -> f32 {
        let cell = ((point - self.origin) / self.pixel).floor();
        if cell.x < 0.0 || cell.y < 0.0 {
            return f32::INFINITY;
        }
        let (x, y) = (cell.x as usize, cell.y as usize);
        if x >= self.width || y >= self.height {
            return f32::INFINITY;
        }
        self.nearest[y * self.width + x]
    }

    /// The stretches of an edge nothing nearer covers, sampled twice a pixel. An edge lies on its
    /// own faces, so it counts as visible within a pixel or two's depth of the nearest surface.
    fn visible_runs(&self, start: (Vec2, f32), end: (Vec2, f32)) -> Vec<(Vec2, Vec2)> {
        let length = (end.0 - start.0).length();
        let steps = ((length / self.pixel) * 2.0).ceil().max(2.0) as usize;
        let tolerance = 0.5 + self.pixel * 2.0;
        let mut runs = Vec::new();
        let mut run: Option<(f32, f32)> = None;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let point = start.0.lerp(end.0, t);
            let depth = start.1 + (end.1 - start.1) * t;
            if depth <= self.at(point) + tolerance {
                run = Some(run.map_or((t, t), |(from, _)| (from, t)));
            } else if let Some((from, to)) = run.take()
                && to > from
            {
                runs.push((start.0.lerp(end.0, from), start.0.lerp(end.0, to)));
            }
        }
        if let Some((from, to)) = run
            && to > from
        {
            runs.push((start.0.lerp(end.0, from), start.0.lerp(end.0, to)));
        }
        runs
    }

    /// The outline of every covered pixel, as closed loops through the midpoints between pixel
    /// centres (marching squares).
    fn outline(&self) -> Vec<Vec<Vec2>> {
        let covered = |x: isize, y: isize| {
            x >= 0
                && y >= 0
                && (x as usize) < self.width
                && (y as usize) < self.height
                && self.nearest[y as usize * self.width + x as usize].is_finite()
        };
        // Points are kept in half-pixel integer units so shared midpoints match exactly.
        let mut next: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
        for y in -1..self.height as isize {
            for x in -1..self.width as isize {
                let (a, b, c, d) = (
                    covered(x, y),
                    covered(x + 1, y),
                    covered(x + 1, y + 1),
                    covered(x, y + 1),
                );
                let (x2, y2) = (2 * x as i64, 2 * y as i64);
                let top = (x2 + 1, y2);
                let right = (x2 + 2, y2 + 1);
                let bottom = (x2 + 1, y2 + 2);
                let left = (x2, y2 + 1);
                let pairs: &[(Half, Half)] = match (a, b, c, d) {
                    (false, false, false, false) | (true, true, true, true) => &[],
                    (true, false, false, false) | (false, true, true, true) => &[(left, top)],
                    (false, true, false, false) | (true, false, true, true) => &[(top, right)],
                    (false, false, true, false) | (true, true, false, true) => &[(right, bottom)],
                    (false, false, false, true) | (true, true, true, false) => &[(bottom, left)],
                    (true, true, false, false) | (false, false, true, true) => &[(left, right)],
                    (false, true, true, false) | (true, false, false, true) => &[(top, bottom)],
                    (true, false, true, false) => &[(left, top), (right, bottom)],
                    (false, true, false, true) => &[(top, right), (bottom, left)],
                };
                for (from, to) in pairs {
                    next.entry(*from).or_default().push(*to);
                    next.entry(*to).or_default().push(*from);
                }
            }
        }
        let mut loops = Vec::new();
        let mut keys: Vec<_> = next.keys().copied().collect();
        keys.sort_unstable();
        for start in keys {
            if next.get(&start).is_none_or(Vec::is_empty) {
                continue;
            }
            let mut ring = vec![start];
            let mut at = start;
            while let Some(to) = next.get_mut(&at).and_then(Vec::pop) {
                if let Some(back) = next.get_mut(&to)
                    && let Some(position) = back.iter().position(|point| *point == at)
                {
                    back.swap_remove(position);
                }
                if to == start {
                    break;
                }
                ring.push(to);
                at = to;
            }
            if ring.len() >= 3 {
                loops.push(
                    ring.into_iter()
                        .map(|(x, y)| {
                            self.origin
                                + (Vec2::new(x as f32, y as f32) * 0.5 + Vec2::splat(0.5))
                                    * self.pixel
                        })
                        .collect(),
                );
            }
        }
        loops
    }
}

/// A point on the outline grid, in half-pixel units.
type Half = (i64, i64);

/// Join segments that share an end into polylines, keeping each part's lines apart.
fn chain(segments: &[(ModelPartKind, Vec2, Vec2)]) -> Vec<(ModelPartKind, Vec<Vec2>)> {
    let key = |point: Vec2| {
        (
            (point.x * 20.0).round() as i64,
            (point.y * 20.0).round() as i64,
        )
    };
    let mut ends: HashMap<(u8, (i64, i64)), Vec<usize>> = HashMap::new();
    let kind_key = |kind: ModelPartKind| kind as u8;
    for (index, (kind, from, to)) in segments.iter().enumerate() {
        ends.entry((kind_key(*kind), key(*from)))
            .or_default()
            .push(index);
        ends.entry((kind_key(*kind), key(*to)))
            .or_default()
            .push(index);
    }
    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();
    for index in 0..segments.len() {
        if used[index] {
            continue;
        }
        used[index] = true;
        let (kind, from, to) = segments[index];
        let mut line = vec![from, to];
        // Grow the line from its end, then from its start.
        for forward in [true, false] {
            loop {
                let tip = if forward {
                    line[line.len() - 1]
                } else {
                    line[0]
                };
                let Some(next) = ends
                    .get(&(kind_key(kind), key(tip)))
                    .and_then(|candidates| candidates.iter().find(|candidate| !used[**candidate]))
                    .copied()
                else {
                    break;
                };
                used[next] = true;
                let (_, a, b) = segments[next];
                let other = if key(a) == key(tip) { b } else { a };
                if forward {
                    line.push(other);
                } else {
                    line.insert(0, other);
                }
            }
        }
        lines.push((kind, line));
    }
    lines
}

/// Douglas–Peucker: drop points that lie within `tolerance` of the line through their neighbours.
fn simplify(points: &[Vec2], tolerance: f32) -> Vec<Vec2> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let (first, last) = (points[0], points[points.len() - 1]);
    let along = last - first;
    let length = along.length();
    let (index, distance) = points[1..points.len() - 1]
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let distance = if length < 1e-6 {
                (*point - first).length()
            } else {
                along.perp_dot(*point - first).abs() / length
            };
            (index + 1, distance)
        })
        .fold(
            (0, -1.0),
            |best, next| if next.1 > best.1 { next } else { best },
        );
    if distance <= tolerance {
        return vec![first, last];
    }
    let mut left = simplify(&points[..=index], tolerance);
    left.pop();
    left.extend(simplify(&points[index..], tolerance));
    left
}

/// Write a drawing as an editable SVG document labelled with its model's id and its view's name.
pub fn drawing_svg(drawing: &ModelDrawing, label: &str, name: &str) -> String {
    let size = (drawing.max - drawing.min).max(Vec2::splat(1.0));
    let margin = (size.max_element() * 0.03).max(2.0);
    let low = drawing.min - Vec2::splat(margin);
    let span = size + Vec2::splat(margin * 2.0);
    let stroke = (size.max_element() * 0.0025).clamp(0.3, 2.0);
    let number = |value: f32| {
        let text = format!("{value:.2}");
        let text = text.trim_end_matches('0').trim_end_matches('.');
        if text == "-0" {
            "0".to_owned()
        } else {
            text.to_owned()
        }
    };
    let path = |points: &[Vec2], closed: bool| {
        let mut d = String::new();
        for (index, point) in points.iter().enumerate() {
            let command = if index == 0 { 'M' } else { 'L' };
            let _ = write!(d, "{command}{} {} ", number(point.x), number(point.y));
        }
        if closed {
            d.push('Z');
        }
        d.trim_end().to_owned()
    };
    let polyline = |line: &[Vec2]| {
        let closed = line.len() > 2 && (line[0] - line[line.len() - 1]).length() < 0.05;
        let points = if closed {
            &line[..line.len() - 1]
        } else {
            line
        };
        format!(r#"      <path d="{}"/>"#, path(points, closed))
    };
    let rings = |rings: &[Vec<Vec2>]| {
        rings
            .iter()
            .map(|ring| path(ring, true))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut extra = String::new();
    if let Some(hinge) = drawing.hinge {
        let _ = write!(
            extra,
            r#" data-hinge="{} {}""#,
            number(hinge.x),
            number(hinge.y)
        );
    }
    if let Some(bracket) = drawing
        .bracket_degrees
        .filter(|bracket| bracket.abs() > 1e-3)
    {
        let _ = write!(extra, r#" data-bracket="{}""#, number(bracket));
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}mm" height="{}mm" data-model="{label}" data-view="{name}" data-pose="{}"{extra} data-generator="{DRAWING_GENERATOR_ID}" data-generator-version="{DRAWING_GENERATOR_VERSION}">"#,
        number(low.x),
        number(low.y),
        number(span.x),
        number(span.y),
        number(span.x),
        number(span.y),
        drawing.pose,
    );
    let _ = writeln!(out, "  <title>{label} — {name}</title>");
    if !drawing.hardware_silhouette.is_empty() {
        let _ = writeln!(
            out,
            r##"  <g id="silhouette" fill="#1b1f24" stroke="none"><path id="silhouette-body" fill-rule="evenodd" d="{}"/><path id="silhouette-hardware" fill-rule="evenodd" d="{}"/></g>"##,
            rings(&drawing.silhouette),
            rings(&drawing.hardware_silhouette),
        );
    } else if !drawing.silhouette.is_empty() {
        let _ = writeln!(
            out,
            r##"  <g id="silhouette" fill="#1b1f24" stroke="none"><path fill-rule="evenodd" d="{}"/></g>"##,
            rings(&drawing.silhouette),
        );
    }
    let _ = writeln!(
        out,
        r##"  <g id="lines" fill="none" stroke="#d7dde4" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round">"##,
        number(stroke)
    );
    for (kind, id) in [
        (ModelPartKind::Base, "base"),
        (ModelPartKind::Yoke, "yoke"),
        (ModelPartKind::Head, "head"),
    ] {
        let paths = drawing
            .lines
            .iter()
            .filter(|(line_kind, _)| *line_kind == kind)
            .map(|(_, line)| polyline(line))
            .collect::<Vec<_>>();
        if paths.is_empty() {
            continue;
        }
        let _ = writeln!(out, r#"    <g id="{id}">"#);
        for line in paths {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out, "    </g>");
    }
    if !drawing.hardware_lines.is_empty() {
        let _ = writeln!(out, r#"    <g id="hardware">"#);
        for line in &drawing.hardware_lines {
            let _ = writeln!(out, "{}", polyline(line));
        }
        let _ = writeln!(out, "    </g>");
    }
    let _ = writeln!(out, "  </g>");
    let mark = (size.max_element() * 0.03).clamp(3.0, 20.0);
    let _ = writeln!(
        out,
        r##"  <g id="origin" fill="none" stroke="#e5484d" stroke-width="{}"><path d="M{} 0 H{} M0 {} V{}"/></g>"##,
        number(stroke),
        number(-mark),
        number(mark),
        number(-mark),
        number(mark),
    );
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_scene::ModelPart;

    /// A box as a model part, each face its own four vertices with the face's normal.
    fn cuboid(kind: ModelPartKind, name: &str, min: Vec3, max: Vec3) -> ModelPart {
        let corner = |x: bool, y: bool, z: bool| {
            Vec3::new(
                if x { max.x } else { min.x },
                if y { max.y } else { min.y },
                if z { max.z } else { min.z },
            )
        };
        let faces: [([Vec3; 4], Vec3); 6] = [
            (
                [
                    corner(false, false, true),
                    corner(true, false, true),
                    corner(true, true, true),
                    corner(false, true, true),
                ],
                Vec3::Z,
            ),
            (
                [
                    corner(true, false, false),
                    corner(false, false, false),
                    corner(false, true, false),
                    corner(true, true, false),
                ],
                Vec3::NEG_Z,
            ),
            (
                [
                    corner(true, false, true),
                    corner(true, false, false),
                    corner(true, true, false),
                    corner(true, true, true),
                ],
                Vec3::X,
            ),
            (
                [
                    corner(false, false, false),
                    corner(false, false, true),
                    corner(false, true, true),
                    corner(false, true, false),
                ],
                Vec3::NEG_X,
            ),
            (
                [
                    corner(false, true, true),
                    corner(true, true, true),
                    corner(true, true, false),
                    corner(false, true, false),
                ],
                Vec3::Y,
            ),
            (
                [
                    corner(false, false, false),
                    corner(true, false, false),
                    corner(true, false, true),
                    corner(false, false, true),
                ],
                Vec3::NEG_Y,
            ),
        ];
        let mut part = ModelPart {
            name: name.into(),
            kind,
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            colour: [0.5; 3],
            roughness: 0.5,
            metallic: 0.0,
        };
        for (quad, normal) in faces {
            let base = part.positions.len() as u32;
            part.positions.extend(quad.map(|point| point.to_array()));
            part.normals.extend([normal.to_array(); 4]);
            part.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        part
    }

    fn model(parts: Vec<ModelPart>) -> FixtureModel {
        FixtureModel {
            parts,
            extent: Vec3::splat(0.5),
            head_pivot: Vec3::ZERO,
            emitter_anchor: None,
            emitter_size: None,
            emitter_axis: None,
            has_head: false,
            warnings: Vec::new(),
        }
    }

    fn length(drawing: &ModelDrawing) -> f32 {
        drawing
            .lines
            .iter()
            .flat_map(|(_, line)| line.windows(2).map(|pair| (pair[1] - pair[0]).length()))
            .sum()
    }

    /// A box seen square-on is its outline: four edges, nothing across its face and nothing of the
    /// edges behind it.
    #[test]
    fn a_box_seen_square_on_is_drawn_as_its_outline_alone() {
        let drawing = model_drawing(
            &model(vec![cuboid(
                ModelPartKind::Base,
                "body",
                Vec3::splat(-0.05),
                Vec3::splat(0.05),
            )]),
            ProfileProjectionView::Front,
        )
        .unwrap();
        assert!(
            (length(&drawing) - 400.0).abs() < 2.0,
            "{}",
            length(&drawing)
        );
        let inside = drawing
            .lines
            .iter()
            .flat_map(|(_, line)| line.iter())
            .any(|point| point.x.abs() < 49.0 && point.y.abs() < 49.0);
        assert!(!inside, "nothing is drawn across the face");
        assert_eq!(drawing.silhouette.len(), 1);
        let ring = &drawing.silhouette[0];
        let width = ring.iter().map(|p| p.x).fold(f32::MIN, f32::max)
            - ring.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        assert!((width - 100.0).abs() < 1.0, "{width}");
    }

    /// A box three-quarter hidden behind a bigger one shows only the edges that stick out.
    #[test]
    fn edges_behind_a_nearer_part_are_left_out() {
        let front = cuboid(
            ModelPartKind::Base,
            "front",
            Vec3::new(-0.1, -0.1, 0.0),
            Vec3::new(0.1, 0.1, 0.1),
        );
        let behind = cuboid(
            ModelPartKind::Head,
            "behind",
            Vec3::new(0.05, 0.05, -0.2),
            Vec3::new(0.15, 0.15, -0.1),
        );
        let drawing =
            model_drawing(&model(vec![front, behind]), ProfileProjectionView::Front).unwrap();
        let head: Vec<Vec2> = drawing
            .lines
            .iter()
            .filter(|(kind, _)| *kind == ModelPartKind::Head)
            .flat_map(|(_, line)| line.clone())
            .collect();
        assert!(!head.is_empty(), "the part that sticks out is drawn");
        // Page y runs down, so the front box covers x and y from −100 to 100.
        let covered = head
            .iter()
            .any(|point| point.x < 99.0 && point.x > -99.0 && point.y < 99.0 && point.y > -99.0);
        assert!(
            !covered,
            "nothing of the box behind is drawn over the front box: {head:?}"
        );
    }

    #[test]
    fn a_drawing_is_editable_svg_in_millimetres_with_named_groups() {
        let mut moving = model(vec![
            cuboid(
                ModelPartKind::Base,
                "base",
                Vec3::new(-0.1, 0.0, -0.1),
                Vec3::new(0.1, 0.05, 0.1),
            ),
            cuboid(
                ModelPartKind::Head,
                "head",
                Vec3::new(-0.08, -0.3, -0.08),
                Vec3::new(0.08, -0.05, 0.08),
            ),
        ]);
        moving.has_head = true;
        let svg =
            model_drawing_svg(&moving, "test-head", "top", ProfileProjectionView::Top).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        for needle in [
            "data-model=\"test-head\"",
            "data-view=\"top\"",
            "data-pose=\"moving-forward\"",
            "<g id=\"silhouette\"",
            "<g id=\"lines\"",
            "<g id=\"base\">",
            "<g id=\"origin\"",
            "mm\" height=",
        ] {
            assert!(svg.contains(needle), "missing {needle} in\n{svg}");
        }
        let front =
            model_drawing_svg(&moving, "test-head", "front", ProfileProjectionView::Front).unwrap();
        assert!(front.contains("data-pose=\"moving-down\""));
        assert!(front.contains("<g id=\"head\">"));
    }

    /// A PAR hanging face-down reads as a circle from above; turned about its hinge to point
    /// forward it shows its length, while the hardware it hangs by stays put.
    #[test]
    fn a_body_tilt_turns_the_lamp_about_its_hinge_and_leaves_the_hardware() {
        let lamp = model(vec![
            cuboid(
                ModelPartKind::Base,
                "hanging-frame",
                Vec3::new(-0.1, -0.02, -0.02),
                Vec3::new(0.1, 0.02, 0.02),
            ),
            cuboid(
                ModelPartKind::Base,
                "par-can",
                Vec3::new(-0.05, -0.4, -0.05),
                Vec3::new(0.05, -0.1, 0.05),
            ),
        ]);
        let span = |drawing: &ModelDrawing| {
            let points: Vec<Vec2> = drawing
                .lines
                .iter()
                .map(|(_, line)| line)
                .chain(&drawing.hardware_lines)
                .flat_map(|line| line.clone())
                .collect();
            let max = points.iter().fold(Vec2::splat(f32::MIN), |a, b| a.max(*b));
            let min = points.iter().fold(Vec2::splat(f32::MAX), |a, b| a.min(*b));
            max - min
        };
        let hanging = model_drawing(&lamp, ProfileProjectionView::Top).unwrap();
        assert!(
            span(&hanging).y < 101.0,
            "from above the can is its end: {:?}",
            span(&hanging)
        );
        let tilt = BodyTilt {
            hinge: Vec3::ZERO,
            target: Vec3::Z,
        };
        let forward = model_drawing_with(&lamp, ProfileProjectionView::Top, Some(tilt)).unwrap();
        assert_eq!(forward.pose, "tilted");
        // The can now runs 100–400 mm forward of the hinge; the frame still spans 200 mm across.
        assert!((forward.max.y - 400.0).abs() < 1.0, "{:?}", forward.max);
        assert!(
            (span(&forward).x - 200.0).abs() < 1.0,
            "{:?}",
            span(&forward)
        );
    }

    fn hung_lamp() -> FixtureModel {
        model(vec![
            cuboid(
                ModelPartKind::Base,
                "hanging-frame",
                Vec3::new(-0.1, -0.12, -0.02),
                Vec3::new(0.1, 0.0, 0.02),
            ),
            cuboid(
                ModelPartKind::Base,
                "par-can",
                Vec3::new(-0.05, -0.4, -0.05),
                Vec3::new(0.05, -0.1, 0.05),
            ),
        ])
    }

    /// A hinged drawing keeps the hanging frame in its own group with its own silhouette, and
    /// records the hinge on the page and the bracket angle its drawn pose equals: leaning 20°
    /// toward the viewer is a face-down lamp at bracket −70°.
    #[test]
    fn a_hinged_drawing_keeps_the_hardware_apart_and_records_hinge_and_bracket() {
        let pose = drawing_pose("flat-led-par", "side", Some(Vec3::new(0.0, -0.1, 0.0)));
        let drawing =
            model_drawing_posed(&hung_lamp(), ProfileProjectionView::Left, &pose).unwrap();
        assert_eq!(drawing.hinge, Some(Vec2::new(0.0, 100.0)));
        let bracket = drawing
            .bracket_degrees
            .expect("a turned body records its bracket");
        assert!((bracket + 70.0).abs() < 1e-3, "{bracket}");
        let visualizer = glam::Quat::from_rotation_x(bracket.to_radians()) * Vec3::NEG_Y;
        assert!((visualizer - lean_target(DEFAULT_LEAN_DEGREES)).length() < 1e-4);
        assert!(!drawing.hardware_silhouette.is_empty());
        assert!(!drawing.hardware_lines.is_empty());
        // The frame's top edge, at the mounting point, is hardware, not body.
        let at_top = |line: &Vec<Vec2>| line.iter().any(|point| point.y.abs() < 0.5);
        assert!(drawing.hardware_lines.iter().any(at_top));
        assert!(!drawing.lines.iter().any(|(_, line)| at_top(line)));
        // The body leans forward (+Z, right on the page) from the hinge.
        assert!(drawing.max.x > 280.0, "{:?}", drawing.max);

        let svg = drawing_svg(&drawing, "flat-led-par", "side");
        for needle in [
            "data-hinge=\"0 100\"",
            "data-bracket=\"-70\"",
            "<path id=\"silhouette-body\"",
            "<path id=\"silhouette-hardware\"",
            "<g id=\"hardware\">",
        ] {
            assert!(svg.contains(needle), "missing {needle} in\n{svg}");
        }

        // Without a hinge the drawing is one piece, as before.
        let plain = model_drawing(&hung_lamp(), ProfileProjectionView::Left).unwrap();
        let svg = drawing_svg(&plain, "par", "side");
        assert!(plain.hardware_lines.is_empty() && plain.hinge.is_none());
        for needle in [
            "data-hinge",
            "data-bracket",
            "silhouette-body",
            "id=\"hardware\"",
        ] {
            assert!(!svg.contains(needle), "unexpected {needle} in\n{svg}");
        }
    }

    /// An LED wash is drawn from the front with its head tilted to look at the viewer: its
    /// 250 mm long head then shows its 160 mm face.
    #[test]
    fn an_led_wash_looks_at_the_viewer_from_the_front() {
        let mut wash = model(vec![
            cuboid(
                ModelPartKind::Base,
                "base",
                Vec3::new(-0.1, 0.0, -0.1),
                Vec3::new(0.1, 0.05, 0.1),
            ),
            cuboid(
                ModelPartKind::Head,
                "head",
                Vec3::new(-0.08, -0.3, -0.08),
                Vec3::new(0.08, -0.05, 0.08),
            ),
        ]);
        wash.has_head = true;
        let head_height = |drawing: &ModelDrawing| {
            let ys: Vec<f32> = drawing
                .lines
                .iter()
                .filter(|(kind, _)| *kind == ModelPartKind::Head)
                .flat_map(|(_, line)| line.iter().map(|point| point.y))
                .collect();
            ys.iter().copied().fold(f32::MIN, f32::max)
                - ys.iter().copied().fold(f32::MAX, f32::min)
        };
        let down = model_drawing(&wash, ProfileProjectionView::Front).unwrap();
        assert_eq!(down.pose, "moving-down");
        assert!(
            (head_height(&down) - 250.0).abs() < 2.0,
            "{}",
            head_height(&down)
        );
        let pose = drawing_pose("moving-head-led-wash-400", "front", None);
        let facing = model_drawing_posed(&wash, ProfileProjectionView::Front, &pose).unwrap();
        assert_eq!(facing.pose, "moving-forward");
        assert!(
            (head_height(&facing) - 160.0).abs() < 2.0,
            "{}",
            head_height(&facing)
        );
    }
}
