//! Authored slab outlines and elevations are larger than their editing handle's size.
use crate::{Aabb, SceneryObject, euler_degrees};
use glam::{Mat4, Vec3};
pub(crate) fn expand(bounds: &mut Aabb, object: &SceneryObject) -> bool {
    let Some(architecture) = &object.architecture else {
        return false;
    };
    let Ok(geometry) = light_fixture::venue_geometry(
        architecture,
        object.size.as_dvec3().to_array().map(|v| v * 1000.0),
    ) else {
        return false;
    };
    let pose =
        Mat4::from_rotation_translation(euler_degrees(object.rotation_degrees), object.position);
    for p in geometry.triangles.iter().flatten() {
        bounds.expand(pose.transform_point3(Vec3::from_array(p.map(|v| v as f32 * 0.001))));
    }
    true
}

/// Junction miters can extend beyond the authored editing box.
pub(crate) fn expand_joined(bounds: &mut Aabb, objects: &[SceneryObject]) {
    use light_fixture::{VenueArchitecture, VenueWallPlacement, venue_joined_walls};
    let walls: Vec<_> = objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| {
            let VenueArchitecture::Wall { openings, .. } = object.architecture.as_ref()? else {
                return None;
            };
            let mut pose = Mat4::from_rotation_translation(
                euler_degrees(object.rotation_degrees),
                object.position,
            )
            .to_cols_array_2d()
            .map(|column| column.map(f64::from));
            for value in &mut pose[3][..3] {
                *value *= 1000.0;
            }
            Some(VenueWallPlacement {
                id: if object.id.is_nil() {
                    uuid::Uuid::from_u128(index as u128 + 1)
                } else {
                    object.id
                },
                size_millimetres: object.size.to_array().map(|v| f64::from(v) * 1000.0),
                local_to_world: pose,
                openings: openings.clone(),
            })
        })
        .collect();
    let joined = venue_joined_walls(&walls);
    for wall in walls {
        let Some(geometry) = joined.get(&wall.id) else {
            continue;
        };
        let pose = glam::DMat4::from_cols_array_2d(&wall.local_to_world);
        for point in geometry.triangles.iter().flatten() {
            bounds
                .expand((pose.transform_point3(glam::DVec3::from_array(*point)) * 0.001).as_vec3());
        }
    }
}
