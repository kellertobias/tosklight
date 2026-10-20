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
