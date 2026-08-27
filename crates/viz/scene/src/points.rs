//! Slaving a placement to a 3D Point.
//!
//! A 3D Point is a reference object an operator patches into the rig and then moves from the
//! Position encoders. Fixtures and venue elements can be slaved to one, so flying a truss point
//! or angling it carries everything hung on it.
//!
//! The maths lives here rather than beside the desk's placement resolution because both the desk
//! that builds a scene and the renderer that draws one have to agree on it exactly, and a second
//! answer to where a lantern is would be worse than no answer at all.

use glam::{EulerRot, Quat, Vec3};
use uuid::Uuid;

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
    let turn = Quat::from_euler(
        EulerRot::YXZ,
        turn_degrees.y.to_radians(),
        turn_degrees.x.to_radians(),
        turn_degrees.z.to_radians(),
    );
    (
        origin + turn * (position - origin) + offset,
        // The fixture turns with the point it hangs on, so its own aim rides on top.
        rotation_degrees + turn_degrees,
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
        assert_eq!(rotation, Vec3::new(0.0, 90.0, 0.0));
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
