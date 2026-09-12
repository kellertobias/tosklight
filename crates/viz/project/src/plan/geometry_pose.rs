//! Posing a packaged model by the fixture's own geometry graph.
//!
//! A GLB arrives as one flat set of parts. The graph says where each named part actually sits and
//! what it turns about, and baking that in is what keeps a moving head's head out of its base.

use glam::{EulerRot, Mat4, Quat, Vec3};
use light_fixture::GeometryGraph;
use std::collections::HashMap;
use uuid::Uuid;

/// Put model parts onto the authoritative profile geometry graph before the renderer animates
/// them. Package GLBs keep `moving-base`, `moving-yoke`, and `moving-head` as reusable local
/// subtrees; the graph supplies their real offsets and pivots. Flattening the GLB without this
/// step left the head inside the base on the ROBE and JB moving lights.
pub(super) fn apply_profile_model_pose(
    model: &mut viz_scene::FixtureModel,
    geometry: &GeometryGraph,
) {
    let bound = geometry
        .nodes
        .iter()
        .filter(|node| node.glb_node.is_some())
        .collect::<Vec<_>>();
    if bound.is_empty() {
        return;
    }

    let mut world = HashMap::<Uuid, Mat4>::new();
    for _ in 0..=bound.len() {
        let mut progressed = false;
        for node in &bound {
            if world.contains_key(&node.id) {
                continue;
            }
            let parent = match node.parent_id {
                Some(parent) => match world.get(&parent) {
                    Some(parent) => *parent,
                    None => continue,
                },
                None => Mat4::IDENTITY,
            };
            world.insert(node.id, parent * profile_geometry_transform(node));
            progressed = true;
        }
        if !progressed {
            break;
        }
    }

    let mut transforms = HashMap::new();
    for node in bound {
        let Some(name) = node.glb_node.as_deref() else {
            continue;
        };
        let Some(transform) = world.get(&node.id).copied() else {
            continue;
        };
        transforms.insert(viz_scene::ModelPartKind::from_node_name(name), transform);
    }
    if transforms
        .values()
        .all(|matrix| matrix.abs_diff_eq(Mat4::IDENTITY, 1e-6))
    {
        return;
    }

    for part in &mut model.parts {
        let Some(transform) = transforms.get(&part.kind).copied() else {
            continue;
        };
        for position in &mut part.positions {
            *position = transform
                .transform_point3(Vec3::from_array(*position))
                .to_array();
        }
        for normal in &mut part.normals {
            *normal = transform
                .transform_vector3(Vec3::from_array(*normal))
                .normalize_or(Vec3::Y)
                .to_array();
        }
    }

    if let Some(head) = transforms.get(&viz_scene::ModelPartKind::Head).copied() {
        model.head_pivot = head.transform_point3(model.head_pivot);
        model.emitter_anchor = model
            .emitter_anchor
            .map(|anchor| head.transform_point3(anchor));
        model.emitter_axis = model
            .emitter_axis
            .map(|axis| head.transform_vector3(axis).normalize_or(Vec3::NEG_Y));
    }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for point in model
        .parts
        .iter()
        .flat_map(|part| part.positions.iter().copied())
        .map(Vec3::from_array)
    {
        min = min.min(point);
        max = max.max(point);
    }
    if min.x <= max.x {
        model.extent = ((max - min) * 0.5).max(Vec3::splat(0.001));
    }
}

fn profile_geometry_transform(node: &light_fixture::GeometryNode) -> Mat4 {
    let translation = Vec3::new(
        node.transform.translation.x,
        node.transform.translation.y,
        node.transform.translation.z,
    ) / 1_000.0;
    let pivot = Vec3::new(node.pivot.x, node.pivot.y, node.pivot.z) / 1_000.0;
    let rotation = &node.transform.rotation_degrees;
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        rotation.x.to_radians(),
        rotation.y.to_radians(),
        rotation.z.to_radians(),
    );
    let read_scale = |value: f32| if value == 0.0 { 1.0 } else { value };
    let scale = Vec3::new(
        read_scale(node.transform.scale.x),
        read_scale(node.transform.scale.y),
        read_scale(node.transform.scale.z),
    );
    Mat4::from_translation(translation + pivot)
        * Mat4::from_quat(rotation)
        * Mat4::from_scale(scale)
        * Mat4::from_translation(-pivot)
}
