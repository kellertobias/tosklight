//! Slaving a placement to a 3D Point.
//!
//! A 3D Point is a reference object an operator patches into the rig and then moves from the
//! Position encoders. Fixtures and venue elements can be slaved to one, so flying a truss point
//! or angling it carries everything hung on it.
//!
//! The maths lives here rather than beside the desk's placement resolution because both the desk
//! that builds a scene and the renderer that draws one have to agree on it exactly, and a second
//! answer to where a lantern is would be worse than no answer at all.

use glam::{EulerRot, Vec3};
use uuid::Uuid;

/// Change the desk XYZ rotation into the renderer basis once, preserving composition.
pub fn desk_rotation_to_world(degrees: [f32; 3]) -> Vec3 {
    let rotation = light_core::spatial::RigidTransform::euler_xyz(degrees.map(f64::from))
        .unwrap_or(light_core::spatial::RigidTransform::IDENTITY);
    Vec3::from(
        rotation
            .desk_pose_to_profile()
            .euler_xyz_degrees()
            .map(|v| v as f32),
    )
}

/// One 3D Point's live pose.
///
/// A point rests at its patched origin, so an untouched point carries a zero offset and no
/// rotation and leaves everything slaved to it exactly where the rig put it.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PointPose {
    pub fixture_id: Uuid,
    /// Where the point itself sits when the show is loaded, in renderer world metres.
    pub origin_metres: [f32; 3],
    /// How far the operator has since moved it.
    pub offset_metres: [f32; 3],
    /// How far the operator has since turned it, in degrees about its own origin.
    pub rotation_degrees: [f32; 3],
}

/// Carry one placement with the point it is slaved to.
///
/// The slave keeps the placement the rig gave it. The point turns it about the point's own origin
/// and then moves it, so a lantern hung two metres stage-left of a truss stays two metres
/// stage-left of it however the truss is flown or angled. Rotating first and translating second is
/// what makes the offset read as "where it sits on the point" rather than "where it sits on the
/// stage".
pub fn slaved_to_point(position: Vec3, rotation_degrees: Vec3, pose: &PointPose) -> (Vec3, Vec3) {
    let origin = Vec3::from(pose.origin_metres);
    let offset = Vec3::from(pose.offset_metres);
    let turn_degrees = Vec3::from(pose.rotation_degrees);
    let turn = crate::scene::euler_degrees(turn_degrees);
    let mounted = turn * crate::scene::euler_degrees(rotation_degrees);
    let (x, y, z) = mounted.to_euler(EulerRot::XYZ);
    (
        origin + turn * (position - origin) + offset,
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(origin: [f32; 3], offset: [f32; 3], rotation: [f32; 3]) -> PointPose {
        PointPose {
            fixture_id: Uuid::nil(),
            origin_metres: origin,
            offset_metres: offset,
            rotation_degrees: rotation,
        }
    }

    #[test]
    fn an_untouched_point_leaves_its_slaves_where_the_rig_put_them() {
        let rigged = Vec3::new(2.0, 6.0, -1.0);
        let (position, rotation) = slaved_to_point(
            rigged,
            Vec3::ZERO,
            &pose([0.0, 6.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        );
        assert_eq!(position, rigged);
        assert_eq!(rotation, Vec3::ZERO);
    }

    #[test]
    fn moving_a_point_carries_its_slaves_the_same_distance() {
        let (position, _) = slaved_to_point(
            Vec3::new(2.0, 6.0, -1.0),
            Vec3::ZERO,
            &pose([0.0, 6.0, 0.0], [0.0, -1.5, 0.0], [0.0, 0.0, 0.0]),
        );
        // Flying the truss down takes the lantern with it and changes nothing about where it
        // sits along the truss.
        assert!((position - Vec3::new(2.0, 4.5, -1.0)).length() < 1e-4);
    }

    #[test]
    fn turning_a_point_swings_its_slaves_about_the_point_not_the_stage() {
        let master = pose([2.0, 6.0, 0.0], [0.0, 0.0, 0.0], [0.0, 90.0, 0.0]);
        let (position, rotation) = slaved_to_point(Vec3::new(4.0, 6.0, 0.0), Vec3::ZERO, &master);
        // A quarter turn about the point keeps it two metres from the point. Turning about the
        // stage origin would have thrown it four metres out instead.
        assert!((position - Vec3::new(2.0, 6.0, -2.0)).length() < 1e-4);
        assert!(((position - Vec3::from(master.origin_metres)).length() - 2.0).abs() < 1e-4);
        assert!(
            crate::scene::euler_degrees(rotation)
                .abs_diff_eq(crate::scene::euler_degrees(Vec3::new(0., 90., 0.)), 1e-5)
        );
    }

    #[test]
    fn a_point_turns_its_slaves_before_it_moves_them() {
        let (position, _) = slaved_to_point(
            Vec3::new(4.0, 6.0, 0.0),
            Vec3::ZERO,
            &pose([2.0, 6.0, 0.0], [0.0, 0.0, -3.0], [0.0, 90.0, 0.0]),
        );
        // Turn about the point first, then move the whole assembly upstage.
        assert!((position - Vec3::new(2.0, 6.0, -5.0)).length() < 1e-4);
    }
}

#[cfg(test)]
mod compound_tests {
    use super::*;
    #[test]
    fn compound_point_rotation_composes_mount_and_round_trips_to_xyz() {
        let point_rotation = Vec3::new(30., -55., 80.);
        let mount_rotation = Vec3::new(65., 28., -42.);
        let pose = PointPose {
            rotation_degrees: point_rotation.to_array(),
            origin_metres: [1., 2., 3.],
            offset_metres: [0., 2., 0.],
            ..Default::default()
        };
        let position = Vec3::new(3., 4., 5.);
        let (actual, rotation) = slaved_to_point(position, mount_rotation, &pose);
        let turn = crate::scene::euler_degrees(point_rotation);
        let expected = turn * crate::scene::euler_degrees(mount_rotation);
        assert!(
            (actual
                - (Vec3::from(pose.origin_metres)
                    + turn * (position - Vec3::from(pose.origin_metres))
                    + Vec3::from(pose.offset_metres)))
            .length()
                < 1e-5
        );
        let reconstructed = crate::scene::euler_degrees(rotation);
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!((reconstructed * axis - expected * axis).length() < 1e-5);
        }
        assert!(
            (crate::scene::euler_degrees(point_rotation + mount_rotation) * Vec3::Y
                - expected * Vec3::Y)
                .length()
                > 0.1
        );
    }
}
