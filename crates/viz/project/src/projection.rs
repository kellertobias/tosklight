//! Deterministic orthographic SVG drawings derived from fixture-package GLB geometry.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use glam::{Quat, Vec2, Vec3};
use light_fixture::{
    FixtureProfile, ModelUnits, ProfileProjectionAsset, ProfileProjectionPose,
    ProfileProjectionSet, ProfileProjectionView,
};
use sha2::{Digest, Sha256};
use viz_scene::{FixtureModel, ModelPartKind};

pub const GENERATOR_ID: &str = "tosklight.fixture-projection";
pub const GENERATOR_VERSION: &str = "2";
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
    depth: f32,
    part: ModelPartKind,
    fill: &'static str,
}

/// The two deterministic model poses needed by the interactive orthographic CAD views.
/// Top views point a moving head forwards; every elevation view points it down.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProjectionPose {
    Top,
    Elevation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveProjectionTriangle {
    pub points_millimetres: [[f32; 3]; 3],
    pub colour: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveProjectionMesh {
    pub pose: LiveProjectionPose,
    pub triangles: Vec<LiveProjectionTriangle>,
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

/// Return simplified model-space triangles for live CAD projection. Unlike the portable SVG
/// views, these retain depth so the client can apply the fixture's complete authored rotation
/// before projecting and sorting the surfaces for each viewport.
pub fn generate_live_projection_meshes(
    profile: &FixtureProfile,
) -> Result<Vec<LiveProjectionMesh>, ProjectionError> {
    let source = profile
        .model_asset
        .as_deref()
        .ok_or_else(|| ProjectionError("fixture profile has no 3D model asset".into()))?;
    let bytes = decode_model(source)?;
    let model = viz_scene::read_glb(&bytes).map_err(|error| ProjectionError(error.0))?;
    let scale = physical_scale(profile, &model) * 1000.0;
    [LiveProjectionPose::Top, LiveProjectionPose::Elevation]
        .into_iter()
        .map(|pose| live_projection_mesh(&model, pose, scale))
        .collect()
}

fn live_projection_mesh(
    model: &FixtureModel,
    pose: LiveProjectionPose,
    scale_millimetres: f32,
) -> Result<LiveProjectionMesh, ProjectionError> {
    let target_axis = model.has_head.then_some(match pose {
        LiveProjectionPose::Top => Vec3::Z,
        LiveProjectionPose::Elevation => Vec3::NEG_Y,
    });
    let head_rotation = head_rotation(model, target_axis);
    let mut triangles = Vec::new();
    for part in &model.parts {
        if simplified_away(&part.name) {
            continue;
        }
        let transform = |point: Vec3| {
            (if part.kind == ModelPartKind::Head {
                model.head_pivot + head_rotation * (point - model.head_pivot)
            } else {
                point
            }) * scale_millimetres
        };
        for indices in part.indices.chunks_exact(3) {
            let (Some(first), Some(second), Some(third)) = (
                part.positions.get(indices[0] as usize).copied(),
                part.positions.get(indices[1] as usize).copied(),
                part.positions.get(indices[2] as usize).copied(),
            ) else {
                continue;
            };
            let points = [first, second, third].map(|point| transform(Vec3::from_array(point)));
            if (points[1] - points[0])
                .cross(points[2] - points[0])
                .length_squared()
                < 0.0001
            {
                continue;
            }
            triangles.push(LiveProjectionTriangle {
                points_millimetres: points.map(|point| point.to_array()),
                colour: fill_rgb(part.kind, part.colour),
            });
        }
    }
    if triangles.is_empty() {
        return Err(ProjectionError(
            "3D model contains no usable major geometry".into(),
        ));
    }
    Ok(LiveProjectionMesh { pose, triangles })
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
    let head_rotation = head_rotation(model, target_axis);
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
    for part in &model.parts {
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
            for point in page {
                min = min.min(point);
                max = max.max(point);
            }
            let area = (page[1] - page[0]).perp_dot(page[2] - page[0]).abs() * 0.5;
            if area < 0.04 {
                continue;
            }
            triangles.push(Triangle {
                points: page,
                depth: world
                    .into_iter()
                    .map(|point| depth(point, view))
                    .sum::<f32>()
                    / 3.0,
                part: part.kind,
                fill: fill(part.kind, part.colour),
            });
        }
    }
    if triangles.is_empty() {
        triangles = end_on_bounds_fallback(min, max, view)?;
    }
    triangles.sort_by(|left, right| {
        right
            .depth
            .total_cmp(&left.depth)
            .then_with(|| part_order(left.part).cmp(&part_order(right.part)))
            .then_with(|| point_key(left.points).cmp(&point_key(right.points)))
    });
    let size = (max - min).max(Vec2::splat(0.01));
    let svg = svg(view, pose, min, size, &triangles);
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

fn head_rotation(model: &FixtureModel, target_axis: Option<Vec3>) -> Quat {
    target_axis.map_or(Quat::IDENTITY, |target| {
        Quat::from_rotation_arc(
            model
                .emitter_axis
                .unwrap_or(Vec3::NEG_Y)
                .normalize_or(Vec3::NEG_Y),
            target,
        )
    })
}

fn end_on_bounds_fallback(
    min: Vec2,
    max: Vec2,
    view: ProfileProjectionView,
) -> Result<Vec<Triangle>, ProjectionError> {
    if !min.is_finite() || !max.is_finite() || (max - min).min_element() <= 0.001 {
        return Err(ProjectionError(format!(
            "{} projection contains no visible major geometry",
            view.wire()
        )));
    }
    const SEGMENTS: usize = 16;
    let centre = (min + max) / 2.0;
    let radius = (max - min) / 2.0;
    let ring = (0..SEGMENTS)
        .map(|index| {
            let angle = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            centre + Vec2::new(angle.cos(), angle.sin()) * radius
        })
        .collect::<Vec<_>>();
    Ok((0..SEGMENTS)
        .map(|index| Triangle {
            points: [centre, ring[index], ring[(index + 1) % SEGMENTS]],
            depth: 0.0,
            part: ModelPartKind::Base,
            fill: "#555b64",
        })
        .collect())
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

fn depth(point: Vec3, view: ProfileProjectionView) -> f32 {
    match view {
        ProfileProjectionView::Top => -point.y,
        ProfileProjectionView::Left => point.x,
        ProfileProjectionView::Right => -point.x,
        ProfileProjectionView::Front => -point.z,
        ProfileProjectionView::Back => point.z,
    }
}

fn fill(kind: ModelPartKind, colour: [f32; 3]) -> &'static str {
    let light = colour.into_iter().sum::<f32>() / 3.0;
    match (kind, light > 0.35) {
        (ModelPartKind::Base, false) => "#34383f",
        (ModelPartKind::Base, true) => "#555b64",
        (ModelPartKind::Yoke, false) => "#454a53",
        (ModelPartKind::Yoke, true) => "#676e78",
        (ModelPartKind::Head, false) => "#565c66",
        (ModelPartKind::Head, true) => "#7a828d",
    }
}

fn fill_rgb(kind: ModelPartKind, colour: [f32; 3]) -> [f32; 3] {
    match fill(kind, colour) {
        "#34383f" => [52.0 / 255.0, 56.0 / 255.0, 63.0 / 255.0],
        "#555b64" => [85.0 / 255.0, 91.0 / 255.0, 100.0 / 255.0],
        "#454a53" => [69.0 / 255.0, 74.0 / 255.0, 83.0 / 255.0],
        "#676e78" => [103.0 / 255.0, 110.0 / 255.0, 120.0 / 255.0],
        "#565c66" => [86.0 / 255.0, 92.0 / 255.0, 102.0 / 255.0],
        "#7a828d" => [122.0 / 255.0, 130.0 / 255.0, 141.0 / 255.0],
        _ => unreachable!("fixture projection fill palette is closed"),
    }
}

fn inset_triangle(points: [Vec2; 3]) -> [Vec2; 3] {
    const OUTLINE_WIDTH_MILLIMETRES: f32 = 1.25;
    const MAX_INSET_FRACTION: f32 = 0.18;
    let centre = points.into_iter().sum::<Vec2>() / 3.0;
    points.map(|point| {
        let toward_centre = centre - point;
        let fraction =
            (OUTLINE_WIDTH_MILLIMETRES / toward_centre.length().max(0.001)).min(MAX_INSET_FRACTION);
        point + toward_centre * fraction
    })
}

fn part_order(kind: ModelPartKind) -> u8 {
    match kind {
        ModelPartKind::Base => 0,
        ModelPartKind::Yoke => 1,
        ModelPartKind::Head => 2,
    }
}

fn point_key(points: [Vec2; 3]) -> [i32; 6] {
    [
        (points[0].x * 1000.0) as i32,
        (points[0].y * 1000.0) as i32,
        (points[1].x * 1000.0) as i32,
        (points[1].y * 1000.0) as i32,
        (points[2].x * 1000.0) as i32,
        (points[2].y * 1000.0) as i32,
    ]
}

fn svg(
    view: ProfileProjectionView,
    pose: ProfileProjectionPose,
    min: Vec2,
    size: Vec2,
    triangles: &[Triangle],
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
    for triangle in triangles {
        output.push_str(&format!(
            "<path d=\"M {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3} Z\" fill=\"#171b20\" fill-rule=\"nonzero\" data-part=\"{}-{pose}-outline\"/>",
            triangle.points[0].x,
            triangle.points[0].y,
            triangle.points[1].x,
            triangle.points[1].y,
            triangle.points[2].x,
            triangle.points[2].y,
            match triangle.part {
                ModelPartKind::Base => "base",
                ModelPartKind::Yoke => "yoke",
                ModelPartKind::Head => "head",
            },
        ));
        let inset = inset_triangle(triangle.points);
        output.push_str(&format!(
            "<path d=\"M {:.3} {:.3} L {:.3} {:.3} L {:.3} {:.3} Z\" fill=\"{}\" fill-rule=\"nonzero\" data-part=\"{}-{pose}\"/>",
            inset[0].x,
            inset[0].y,
            inset[1].x,
            inset[1].y,
            inset[2].x,
            inset[2].y,
            triangle.fill,
            match triangle.part {
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
    use std::path::Path;

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

    fn shipped_profile(filename: &str) -> FixtureProfile {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("assets/fixture-library")
            .join(filename);
        light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn generation_is_byte_repeatable_and_records_all_five_poses() {
        let mut profile = shipped_profile("robe--robin-dls-profile.toskfixture");
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
            assert!(svg.contains("-outline\""));
            assert!(svg.contains("fill=\"#171b20\""));
            let paths = svg
                .split("<path")
                .skip(1)
                .map(|path| path.split_once("/>").expect("complete path").0)
                .collect::<Vec<_>>();
            assert_eq!(paths.len() % 2, 0, "outline/fill path pairs");
            for pair in paths.chunks_exact(2) {
                assert!(pair[0].contains("fill=\"#171b20\""));
                assert!(!pair[1].contains("fill=\"#171b20\""));
            }
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
    fn live_meshes_retain_three_dimensional_geometry_for_both_cad_poses() {
        let profile = shipped_profile("robe--robin-dls-profile.toskfixture");
        let first = generate_live_projection_meshes(&profile).expect("live meshes generate");
        let second = generate_live_projection_meshes(&profile).expect("live meshes repeat");
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].pose, LiveProjectionPose::Top);
        assert_eq!(first[1].pose, LiveProjectionPose::Elevation);
        assert!(first.iter().all(|mesh| mesh.triangles.len() > 10));
        assert!(
            first
                .iter()
                .flat_map(|mesh| &mesh.triangles)
                .all(|triangle| {
                    triangle
                        .points_millimetres
                        .iter()
                        .flatten()
                        .all(|coordinate| coordinate.is_finite())
                })
        );
        assert_ne!(first[0].triangles, first[1].triangles);
    }

    #[test]
    fn shipped_railing_projects_posts_rails_and_toe_board_in_every_view() {
        let profile = shipped_profile("venue--stage-railing-2-m.toskfixture");
        let projections = generate_profile_projections(&profile).expect("railing projections");
        assert_eq!(projections.views.len(), 5);
        for projection in projections.views {
            let svg = projection
                .artwork_asset
                .strip_prefix("data:image/svg+xml;base64,")
                .and_then(|encoded| STANDARD.decode(encoded).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .expect("generated railing SVG");
            assert!(
                svg.matches("<path").count() > 20,
                "{} railing projection collapsed to insufficient geometry",
                projection.view.wire()
            );
            assert!(svg.contains("-outline\""));
            assert!(projection.physical_width_millimetres > 0.0);
            assert!(projection.physical_height_millimetres > 0.0);
        }
    }

    #[test]
    fn shipped_pipe_keeps_a_round_end_view_when_its_surface_mesh_is_edge_on() {
        let profile = shipped_profile("venue--one-point-truss-pipe.toskfixture");
        let projections = generate_profile_projections(&profile).expect("pipe projections");
        for view in [ProfileProjectionView::Left, ProfileProjectionView::Right] {
            let projection = projections
                .views
                .iter()
                .find(|projection| projection.view == view)
                .expect("end view");
            let svg = projection
                .artwork_asset
                .strip_prefix("data:image/svg+xml;base64,")
                .and_then(|encoded| STANDARD.decode(encoded).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .expect("generated pipe SVG");
            assert_eq!(svg.matches("-outline\"").count(), 16);
        }
    }
}
