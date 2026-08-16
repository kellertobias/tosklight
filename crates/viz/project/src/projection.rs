//! Deterministic orthographic SVG drawings derived from fixture-package GLB geometry.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use glam::{Quat, Vec2, Vec3};
use light_fixture::{
    FixtureProfile, ModelUnits, ProfileProjectionAsset, ProfileProjectionPose,
    ProfileProjectionSet, ProfileProjectionView,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use viz_scene::{FixtureModel, ModelPartKind};

pub const GENERATOR_ID: &str = "tosklight.fixture-projection";
pub const GENERATOR_VERSION: &str = "16";
pub const POSE_CONTRACT_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionError(pub String);

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProjectionError {}

#[derive(Clone)]
struct Triangle {
    points: [Vec2; 3],
    part: ModelPartKind,
    source_part: usize,
    /// Smaller values are closer to the named orthographic camera, one per projected vertex.
    depths: [f32; 3],
}

#[derive(Clone)]
struct Edge {
    from: Vec2,
    to: Vec2,
    part: ModelPartKind,
    depths: Vec<f32>,
}

pub fn generate_profile_projections(
    profile: &FixtureProfile,
) -> Result<ProfileProjectionSet, ProjectionError> {
    let source = profile
        .model_asset
        .as_deref()
        .ok_or_else(|| ProjectionError("fixture profile has no 3D model asset".into()))?;
    let bytes = decode_model(source)?;
    let model = viz_scene::read_glb(&bytes).map_err(|error| ProjectionError(error.0))?;
    let scale = physical_scale(profile, &model);
    let source_model_sha256 = sha256(&bytes);
    let views = ProfileProjectionView::ALL
        .into_iter()
        .map(|view| generate_view(&model, view, scale))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProfileProjectionSet {
        source_model_sha256,
        generator: GENERATOR_ID.into(),
        generator_version: GENERATOR_VERSION.into(),
        pose_contract_version: POSE_CONTRACT_VERSION,
        views,
    })
}

/// Produce one transient orthographic drawing for a renderer-owned default model.
///
/// Unlike [`generate_profile_projections`], this output is not a package asset: there is no
/// profile-owned source hash to retain. It lets the plan renderer use the same physical SVG
/// geometry for a fallback model as it does for a package model.
pub fn generate_default_model_projection(
    model: &FixtureModel,
    view: ProfileProjectionView,
) -> Result<ProfileProjectionAsset, ProjectionError> {
    generate_view(model, view, 1.0)
}

pub fn projection_cache_is_current(profile: &FixtureProfile) -> bool {
    let Some(source) = profile.model_asset.as_deref() else {
        return false;
    };
    let Ok(bytes) = decode_model(source) else {
        return false;
    };
    profile
        .projection_assets
        .as_ref()
        .is_some_and(|projections| {
            projections.source_model_sha256 == sha256(&bytes)
                && projections.generator == GENERATOR_ID
                && projections.generator_version == GENERATOR_VERSION
                && projections.pose_contract_version == POSE_CONTRACT_VERSION
        })
}

fn decode_model(value: &str) -> Result<Vec<u8>, ProjectionError> {
    let payload = value
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
        .filter(|(metadata, _)| {
            matches!(
                metadata.strip_suffix(";base64"),
                Some("model/gltf-binary" | "application/octet-stream")
            )
        })
        .ok_or_else(|| ProjectionError("3D model must be a self-contained GLB data URL".into()))?;
    STANDARD
        .decode(payload.1)
        .map_err(|error| ProjectionError(format!("3D model data URL is invalid: {error}")))
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn physical_scale(profile: &FixtureProfile, model: &FixtureModel) -> f32 {
    if profile.model_units == ModelUnits::Metres {
        return 1.0;
    }
    let physical = &profile.physical;
    match (
        physical.width_millimetres,
        physical.height_millimetres,
        physical.depth_millimetres,
    ) {
        (Some(width), Some(height), Some(depth)) => {
            model.scale_to(Vec3::new(width, height, depth) / 1000.0)
        }
        _ => 1.0,
    }
}

fn generate_view(
    model: &FixtureModel,
    view: ProfileProjectionView,
    scale: f32,
) -> Result<ProfileProjectionAsset, ProjectionError> {
    let target_axis = match view {
        ProfileProjectionView::Top if model.has_head => Some(Vec3::Z),
        _ if model.has_head => Some(Vec3::NEG_Y),
        _ => None,
    };
    let head_rotation = target_axis.map_or(Quat::IDENTITY, |target| {
        Quat::from_rotation_arc(
            model
                .emitter_axis
                .unwrap_or(Vec3::NEG_Y)
                .normalize_or(Vec3::NEG_Y),
            target,
        )
    });
    let pose = if !model.has_head {
        ProfileProjectionPose::AuthoredHome
    } else if view == ProfileProjectionView::Top {
        ProfileProjectionPose::MovingForward
    } else {
        ProfileProjectionPose::MovingDown
    };
    let mut triangles = Vec::new();
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for (source_part, part) in model.parts.iter().enumerate() {
        if simplified_away(&part.name) {
            continue;
        }
        let transform = |point: Vec3| {
            (if part.kind == ModelPartKind::Head {
                model.head_pivot + head_rotation * (point - model.head_pivot)
            } else {
                point
            }) * scale
        };
        for indices in part.indices.chunks_exact(3) {
            let (Some(first), Some(second), Some(third)) = (
                part.positions.get(indices[0] as usize).copied(),
                part.positions.get(indices[1] as usize).copied(),
                part.positions.get(indices[2] as usize).copied(),
            ) else {
                continue;
            };
            let points = [first, second, third];
            let world = points.map(|point| transform(Vec3::from_array(point)));
            let page = world.map(|point| project(point, view) * 1000.0);
            // An edge-on truss or pipe has no filled area in this view, but it still has a
            // useful physical line. Keep every triangle with at least one visible edge.
            let longest_edge = (page[1] - page[0])
                .length()
                .max((page[2] - page[1]).length())
                .max((page[0] - page[2]).length());
            if longest_edge < 0.01 {
                continue;
            }
            for point in page {
                min = min.min(point);
                max = max.max(point);
            }
            triangles.push(Triangle {
                points: page,
                part: part.kind,
                source_part,
                depths: world.map(|point| depth(point, view)),
            });
        }
    }
    if triangles.is_empty() {
        return Err(ProjectionError(format!(
            "{} projection contains no visible major geometry",
            view.wire()
        )));
    }
    let edges = feature_edges(&triangles, view);
    if edges.is_empty() {
        return Err(ProjectionError(format!(
            "{} projection contains no visible feature edges",
            view.wire()
        )));
    }
    let line_width = ((max - min).max_element() * 0.006).clamp(0.8, 3.0);
    let margin = line_width * 0.75;
    min -= Vec2::splat(margin);
    max += Vec2::splat(margin);
    let size = (max - min).max(Vec2::splat(0.01));
    let svg = svg(view, pose, min, size, line_width, &edges);
    Ok(ProfileProjectionAsset {
        view,
        artwork_asset: format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg)),
        view_box_millimetres: [min.x, min.y, size.x, size.y],
        physical_width_millimetres: size.x,
        physical_height_millimetres: size.y,
        origin_millimetres: [0.0, 0.0],
        orientation: view.orientation(),
        pose,
    })
}

fn simplified_away(name: &str) -> bool {
    let folded = name.to_ascii_lowercase();
    [
        "glass", "grille", "grill", "hole", "screw", "bolt", "washer", "texture",
    ]
    .iter()
    .any(|detail| folded.contains(detail))
}

fn project(point: Vec3, view: ProfileProjectionView) -> Vec2 {
    match view {
        ProfileProjectionView::Top => Vec2::new(point.x, point.z),
        ProfileProjectionView::Left => Vec2::new(point.z, -point.y),
        ProfileProjectionView::Right => Vec2::new(-point.z, -point.y),
        ProfileProjectionView::Front => Vec2::new(point.x, -point.y),
        ProfileProjectionView::Back => Vec2::new(-point.x, -point.y),
    }
}

fn view_axis(view: ProfileProjectionView) -> Vec3 {
    match view {
        ProfileProjectionView::Top => Vec3::Y,
        ProfileProjectionView::Left => Vec3::X,
        ProfileProjectionView::Right => Vec3::NEG_X,
        ProfileProjectionView::Front => Vec3::Z,
        ProfileProjectionView::Back => Vec3::NEG_Z,
    }
}

fn depth(point: Vec3, view: ProfileProjectionView) -> f32 {
    -point.dot(view_axis(view))
}

fn feature_edges(triangles: &[Triangle], view: ProfileProjectionView) -> Vec<Edge> {
    // Fixed fixtures retain one outline per authored GLB component. Moving yokes and heads are
    // merged by semantic part so cheeks, lens rings and body facets do not become separate nests
    // of lines. This is geometry-driven and applies to every default model.
    let mut groups = HashMap::<(ModelPartKind, usize), Vec<&Triangle>>::new();
    for triangle in triangles {
        let source = match triangle.part {
            ModelPartKind::Base => triangle.source_part,
            ModelPartKind::Yoke | ModelPartKind::Head => usize::MAX,
        };
        groups
            .entry((triangle.part, source))
            .or_default()
            .push(triangle);
    }
    let overall_min = triangles
        .iter()
        .flat_map(|triangle| triangle.points)
        .fold(Vec2::splat(f32::INFINITY), Vec2::min);
    let overall_max = triangles
        .iter()
        .flat_map(|triangle| triangle.points)
        .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
    let overall_span = (overall_max - overall_min).max_element();
    let minimum_component_span = (overall_span * 0.025).max(3.0);
    let minimum_contour_length = (overall_span * 0.012).max(3.0);
    let mut result = groups
        .into_iter()
        .filter_map(|((part, _), group)| component_silhouette(&group, part, minimum_component_span))
        .flatten()
        .flat_map(|edge| visible_edge_segments(&edge, triangles, view))
        .filter(|edge| (edge.to - edge.from).length() >= minimum_contour_length)
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        let layering = if view == ProfileProjectionView::Top {
            // A hung fixture's base is the foreground plan surface: its outline and brace must
            // cover the yoke/head where they intersect.
            part_order(right.part).cmp(&part_order(left.part))
        } else {
            part_order(left.part).cmp(&part_order(right.part))
        };
        layering.then_with(|| edge_key(left.from, left.to).cmp(&edge_key(right.from, right.to)))
    });
    result
}

/// Split a contour wherever a nearer face covers it. The face is an invisible mask: emitted SVG
/// stays transparent apart from the pencil-like linework.
fn visible_edge_segments(
    edge: &Edge,
    triangles: &[Triangle],
    view: ProfileProjectionView,
) -> Vec<Edge> {
    const SAMPLES: usize = 64;
    let edge_depth = edge.depths.iter().copied().fold(f32::INFINITY, f32::min);
    let mut result = Vec::new();
    let mut visible_start = None;
    for sample in 0..SAMPLES {
        let start = sample as f32 / SAMPLES as f32;
        let end = (sample + 1) as f32 / SAMPLES as f32;
        let midpoint = (start + end) * 0.5;
        let point = edge.from.lerp(edge.to, midpoint);
        // In the hanging top symbol the yoke runs behind the base. The head keeps physical
        // depth testing: it may project beyond the base and must not disappear wholesale.
        let covered_by_base = view == ProfileProjectionView::Top
            && edge.part == ModelPartKind::Yoke
            && triangles.iter().any(|triangle| {
                triangle.part == ModelPartKind::Base && point_in_triangle(point, triangle.points)
            });
        let visible = !triangles.iter().any(|triangle| {
            triangle_depth_at(point, triangle)
                .is_some_and(|triangle_depth| triangle_depth + 0.25 < edge_depth)
        }) && !covered_by_base;
        match (visible_start, visible) {
            (None, true) => visible_start = Some(start),
            (Some(start), false) => {
                result.push(edge_segment(edge, start, end - 1.0 / SAMPLES as f32));
                visible_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = visible_start {
        result.push(edge_segment(edge, start, 1.0));
    }
    result
}

fn edge_segment(edge: &Edge, start: f32, end: f32) -> Edge {
    Edge {
        from: edge.from.lerp(edge.to, start),
        to: edge.from.lerp(edge.to, end),
        part: edge.part,
        depths: edge.depths.clone(),
    }
}

fn component_silhouette(
    triangles: &[&Triangle],
    part: ModelPartKind,
    minimum_span: f32,
) -> Option<Vec<Edge>> {
    let mut points = triangles
        .iter()
        .flat_map(|triangle| triangle.points)
        .collect::<Vec<_>>();
    points.sort_by(|left, right| {
        left.x
            .total_cmp(&right.x)
            .then_with(|| left.y.total_cmp(&right.y))
    });
    points.dedup_by(|left, right| (*left - *right).length_squared() < 0.01);
    if points.len() < 3 {
        return None;
    }
    let min = points
        .iter()
        .copied()
        .fold(Vec2::splat(f32::INFINITY), Vec2::min);
    let max = points
        .iter()
        .copied()
        .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
    if (max - min).max_element() < minimum_span {
        return None;
    }
    let mut lower: Vec<Vec2> = Vec::new();
    for point in &points {
        while lower.len() >= 2
            && (lower[lower.len() - 1] - lower[lower.len() - 2])
                .perp_dot(*point - lower[lower.len() - 1])
                <= 0.0
        {
            lower.pop();
        }
        lower.push(*point);
    }
    let mut upper: Vec<Vec2> = Vec::new();
    for point in points.iter().rev() {
        while upper.len() >= 2
            && (upper[upper.len() - 1] - upper[upper.len() - 2])
                .perp_dot(*point - upper[upper.len() - 1])
                <= 0.0
        {
            upper.pop();
        }
        upper.push(*point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    simplify_polygon(&mut lower, 12);
    let vertex_depth = |point: Vec2| {
        triangles
            .iter()
            .flat_map(|triangle| triangle.points.into_iter().zip(triangle.depths))
            .filter(|(candidate, _)| (*candidate - point).length_squared() < 0.01)
            .map(|(_, depth)| depth)
            .fold(f32::INFINITY, f32::min)
    };
    Some(
        lower
            .iter()
            .copied()
            .zip(lower.iter().copied().cycle().skip(1))
            .take(lower.len())
            .map(|(from, to)| Edge {
                from,
                to,
                part,
                depths: vec![vertex_depth(from), vertex_depth(to)],
            })
            .collect(),
    )
}

fn simplify_polygon(points: &mut Vec<Vec2>, maximum_vertices: usize) {
    while points.len() > maximum_vertices {
        let remove = (0..points.len())
            .min_by(|left, right| {
                polygon_vertex_area(points, *left).total_cmp(&polygon_vertex_area(points, *right))
            })
            .expect("a non-empty polygon has a least-significant vertex");
        points.remove(remove);
    }
}

fn polygon_vertex_area(points: &[Vec2], index: usize) -> f32 {
    let previous = points[(index + points.len() - 1) % points.len()];
    let point = points[index];
    let next = points[(index + 1) % points.len()];
    (point - previous).perp_dot(next - point).abs()
}

fn point_in_triangle(point: Vec2, triangle: [Vec2; 3]) -> bool {
    let sides = [
        (triangle[1] - triangle[0]).perp_dot(point - triangle[0]),
        (triangle[2] - triangle[1]).perp_dot(point - triangle[1]),
        (triangle[0] - triangle[2]).perp_dot(point - triangle[2]),
    ];
    let tolerance = 0.01;
    sides.iter().all(|side| *side >= -tolerance) || sides.iter().all(|side| *side <= tolerance)
}

fn triangle_depth_at(point: Vec2, triangle: &Triangle) -> Option<f32> {
    if !point_in_triangle(point, triangle.points) {
        return None;
    }
    let [first, second, third] = triangle.points;
    let area = (second - first).perp_dot(third - first);
    if area.abs() < 0.0001 {
        return None;
    }
    let weights = [
        (second - point).perp_dot(third - point) / area,
        (third - point).perp_dot(first - point) / area,
        (first - point).perp_dot(second - point) / area,
    ];
    Some(
        weights
            .into_iter()
            .zip(triangle.depths)
            .map(|(weight, depth)| weight * depth)
            .sum(),
    )
}

fn part_order(kind: ModelPartKind) -> u8 {
    match kind {
        ModelPartKind::Base => 0,
        ModelPartKind::Yoke => 1,
        ModelPartKind::Head => 2,
    }
}

fn edge_key(left: Vec2, right: Vec2) -> [i32; 4] {
    // GLB exporters commonly duplicate a shared vertex with tiny floating-point differences.
    // One millimetre is below a readable plan line, and coalesces those tessellation seams.
    let left = [left.x.round() as i32, left.y.round() as i32];
    let right = [right.x.round() as i32, right.y.round() as i32];
    if left <= right {
        [left[0], left[1], right[0], right[1]]
    } else {
        [right[0], right[1], left[0], left[1]]
    }
}

fn outline_fill(kind: ModelPartKind) -> &'static str {
    match kind {
        ModelPartKind::Base => "#aeb7c4",
        ModelPartKind::Yoke => "#c2cad5",
        ModelPartKind::Head => "#d7dde5",
    }
}

fn line_polygon(from: Vec2, to: Vec2, width: f32) -> Option<[Vec2; 4]> {
    let direction = to - from;
    let length = direction.length();
    if length < 0.01 {
        return None;
    }
    let offset = Vec2::new(-direction.y, direction.x) / length * (width * 0.5);
    Some([from + offset, to + offset, to - offset, from - offset])
}

fn svg(
    view: ProfileProjectionView,
    pose: ProfileProjectionPose,
    min: Vec2,
    size: Vec2,
    line_width: f32,
    edges: &[Edge],
) -> Vec<u8> {
    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{:.3} {:.3} {:.3} {:.3}\" width=\"{:.3}mm\" height=\"{:.3}mm\" data-tosklight-view=\"{}\" data-generator=\"{}\" data-generator-version=\"{}\" data-pose-contract-version=\"{}\">",
        min.x,
        min.y,
        size.x,
        size.y,
        size.x,
        size.y,
        view.wire(),
        GENERATOR_ID,
        GENERATOR_VERSION,
        POSE_CONTRACT_VERSION,
    );
    let pose = match pose {
        ProfileProjectionPose::AuthoredHome => "authored-home",
        ProfileProjectionPose::MovingDown => "moving-down",
        ProfileProjectionPose::MovingForward => "moving-forward",
    };
    for edge in edges {
        let Some(points) = line_polygon(edge.from, edge.to, line_width) else {
            continue;
        };
        output.push_str(&format!(
            "<path d=\"M {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3} Z\" fill=\"{}\" fill-rule=\"nonzero\" data-part=\"outline-{}-{pose}\"/>",
            points[0].x,
            points[0].y,
            points[1].x,
            points[1].y,
            points[2].x,
            points[2].y,
            points[3].x,
            points[3].y,
            outline_fill(edge.part),
            match edge.part {
                ModelPartKind::Base => "base",
                ModelPartKind::Yoke => "yoke",
                ModelPartKind::Head => "head",
            },
        ));
    }
    output.push_str("</svg>");
    output.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_views_keep_the_documented_page_axes() {
        let point = Vec3::new(2.0, 3.0, 5.0);
        assert_eq!(
            project(point, ProfileProjectionView::Top),
            Vec2::new(2.0, 5.0)
        );
        assert_eq!(
            project(point, ProfileProjectionView::Left),
            Vec2::new(5.0, -3.0)
        );
        assert_eq!(
            project(point, ProfileProjectionView::Right),
            Vec2::new(-5.0, -3.0)
        );
        assert_eq!(
            project(point, ProfileProjectionView::Front),
            Vec2::new(2.0, -3.0)
        );
        assert_eq!(
            project(point, ProfileProjectionView::Back),
            Vec2::new(-2.0, -3.0)
        );
    }

    #[test]
    fn simplification_is_deliberate_and_name_based() {
        assert!(simplified_away("Lens Glass"));
        assert!(simplified_away("rear grille"));
        assert!(!simplified_away("moving-head-body"));
    }

    #[test]
    fn contour_visibility_masks_an_edge_behind_a_nearer_face() {
        let edge = Edge {
            from: Vec2::new(0.0, 0.0),
            to: Vec2::new(10.0, 0.0),
            part: ModelPartKind::Head,
            depths: vec![10.0],
        };
        let occluding_face = Triangle {
            points: [
                Vec2::new(-1.0, -1.0),
                Vec2::new(11.0, -1.0),
                Vec2::new(5.0, 10.0),
            ],
            part: ModelPartKind::Base,
            source_part: 0,
            depths: [0.0, 0.0, 0.0],
        };
        assert!(
            visible_edge_segments(
                &edge,
                &[occluding_face.clone()],
                ProfileProjectionView::Front
            )
            .is_empty()
        );

        let visible_face = Triangle {
            depths: [11.0, 11.0, 11.0],
            ..occluding_face
        };
        assert_eq!(
            visible_edge_segments(&edge, &[visible_face], ProfileProjectionView::Front).len(),
            1
        );
    }

    #[test]
    fn generation_is_byte_repeatable_and_records_all_five_poses() {
        let mut profile = FixtureProfile::blank();
        profile.fixture_type = "profile moving head".into();
        profile.model_asset = Some(format!(
            "data:model/gltf-binary;base64,{}",
            STANDARD.encode(crate::default_model::MOVING_PROFILE.bytes)
        ));
        let first = generate_profile_projections(&profile).expect("projections generate");
        let second = generate_profile_projections(&profile).expect("regeneration is repeatable");
        assert_eq!(first, second);
        assert_eq!(first.views.len(), 5);
        for view in &first.views {
            let expected = if view.view == ProfileProjectionView::Top {
                ProfileProjectionPose::MovingForward
            } else {
                ProfileProjectionPose::MovingDown
            };
            assert_eq!(view.pose, expected, "{}", view.view.wire());
            let svg = view
                .artwork_asset
                .strip_prefix("data:image/svg+xml;base64,")
                .and_then(|encoded| STANDARD.decode(encoded).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .expect("generated SVG data URL");
            assert!(svg.contains(&format!("data-tosklight-view=\"{}\"", view.view.wire())));
            assert!(!svg.contains("<script"));
        }
        profile.projection_assets = Some(first);
        assert!(projection_cache_is_current(&profile));
        let encoded = profile
            .model_asset
            .as_deref()
            .and_then(|asset| asset.split_once(','))
            .map(|(_, encoded)| encoded)
            .expect("model data URL");
        let mut changed = STANDARD.decode(encoded).expect("model bytes");
        changed.push(0);
        profile.model_asset = Some(format!(
            "data:model/gltf-binary;base64,{}",
            STANDARD.encode(changed)
        ));
        assert!(!projection_cache_is_current(&profile));
    }

    #[test]
    fn every_default_model_has_bounded_complete_orthographic_linework() {
        for shipped in crate::default_model::all() {
            let model = viz_scene::read_glb(shipped.bytes).expect("default model reads");
            for view in ProfileProjectionView::ALL {
                let projection = generate_default_model_projection(&model, view)
                    .unwrap_or_else(|error| panic!("{} {}: {error}", shipped.name, view.wire()));
                let svg = projection
                    .artwork_asset
                    .strip_prefix("data:image/svg+xml;base64,")
                    .and_then(|encoded| STANDARD.decode(encoded).ok())
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .expect("generated default SVG data URL");
                let contours = svg.matches("<path ").count();
                assert!(
                    (4..=120).contains(&contours),
                    "{} {} emitted {contours} contours",
                    shipped.name,
                    view.wire()
                );
                if model.has_head && view != ProfileProjectionView::Top {
                    for part in ["base", "yoke", "head"] {
                        assert!(
                            svg.contains(&format!("data-part=\"outline-{part}-")),
                            "{} {} omitted its {part}",
                            shipped.name,
                            view.wire()
                        );
                    }
                }
            }
        }
    }
}
