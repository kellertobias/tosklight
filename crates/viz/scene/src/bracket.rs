//! The mounting bracket: what stays with the rig and what turns with the lamp.
//!
//! A lantern hangs from its hardware — the hanging frame (stirrup) and the truss coupler — and the
//! bracket angle turns only the body in that frame, about the hinge where the frame's bolts hold
//! it. The CAD draws a hinged side view the same way (`viz_project::model_drawing`), from the same
//! hinge and the same hardware names, so the plan and the 3D view agree on the pose.
//!
//! The angle is about the fixture's own transverse axis (+X, `Quat::from_rotation_x`, positive
//! nose-down), composed after the placement rotation: the bar decides which way the lantern faces
//! and the bracket decides how far down it looks.

use crate::scene::euler_degrees;
use crate::{FixtureInstance, PointPose};
use glam::{Quat, Vec3};

/// Whether a model part is the hardware a lamp hangs by rather than the lamp itself: the hanging
/// frame and the truss coupler (with its bolt and bond). Such parts stay where they hang while the
/// bracket turns the body.
pub fn is_mounting_hardware(name: &str) -> bool {
    let folded = name.to_ascii_lowercase();
    folded.contains("hanging-frame") || folded.contains("truss-coupler")
}

/// The point `local` (fixture metres, in the unturned body) reaches once the bracket turns the
/// body `bracket_degrees` about `hinge`.
pub fn bracket_turned(local: Vec3, hinge: Vec3, bracket_degrees: f32) -> Vec3 {
    hinge + bracket_quat(bracket_degrees) * (local - hinge)
}

fn bracket_quat(bracket_degrees: f32) -> Quat {
    if bracket_degrees.abs() < f32::EPSILON {
        return Quat::IDENTITY;
    }
    Quat::from_rotation_x(bracket_degrees.to_radians())
}

impl FixtureInstance {
    /// The body's rotation: the mounting rotation with the bracket angle on top of it.
    pub fn orientation(&self) -> Quat {
        euler_degrees(self.rotation_degrees) * self.bracket_rotation()
    }

    /// The bracket's own rotation, about the fixture's transverse axis.
    pub fn bracket_rotation(&self) -> Quat {
        bracket_quat(self.bracket_degrees)
    }

    /// Where this instance's mount stands and how it is turned, without the bracket, once the 3D
    /// Point it is slaved to has been applied. The mounting hardware is drawn in this frame.
    pub fn mounted_by(&self, points: &[PointPose]) -> (Vec3, Quat) {
        let Some(master) = self.position_master else {
            return (self.position, euler_degrees(self.rotation_degrees));
        };
        let Some(pose) = points.iter().find(|pose| pose.fixture_id == master) else {
            // The point is gone or has not reported yet: stand where the rig put it rather than
            // guessing at an offset.
            return (self.position, euler_degrees(self.rotation_degrees));
        };
        let (position, rotation_degrees) =
            crate::slaved_to_point(self.position, self.rotation_degrees, pose);
        (position, euler_degrees(rotation_degrees))
    }

    /// The frame the lamp's body is drawn in: the mount turned by the bracket about its hinge.
    ///
    /// Every body draw path asks this rather than reading `position` and `orientation` directly,
    /// so a slaved or bracketed lantern's body, its yoke and its beam can never disagree about
    /// where it is. A body point `p` lands at `position + orientation * p`, which is the mount
    /// frame applied to `hinge + bracket * (p − hinge)`.
    pub fn placed_by(&self, points: &[PointPose]) -> (Vec3, Quat) {
        let (position, mount) = self.mounted_by(points);
        let hinge = self.bracket_hinge.unwrap_or(Vec3::ZERO);
        let offset = hinge - self.bracket_rotation() * hinge;
        (position + mount * offset, mount * self.bracket_rotation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lantern(bracket: f32, hinge: Option<Vec3>) -> FixtureInstance {
        FixtureInstance {
            position: Vec3::new(1.0, 5.0, -2.0),
            rotation_degrees: Vec3::new(0.0, 90.0, 0.0),
            bracket_degrees: bracket,
            bracket_hinge: hinge,
            ..FixtureInstance::default()
        }
    }

    #[test]
    fn the_body_turns_about_the_hinge_and_the_mount_stays() {
        let hinge = Vec3::new(0.0, -0.35, 0.0);
        let fixture = lantern(45.0, Some(hinge));
        let (mount_position, mount) = fixture.mounted_by(&[]);
        assert_eq!(mount_position, fixture.position);
        assert!(mount.abs_diff_eq(euler_degrees(fixture.rotation_degrees), 1e-6));

        let (body_position, body) = fixture.placed_by(&[]);
        // The hinge itself does not move.
        let hinge_world = body_position + body * hinge;
        assert!((hinge_world - (mount_position + mount * hinge)).length() < 1e-5);
        // The lens below the hinge swings 45° about the transverse axis.
        let lens = Vec3::new(0.0, -0.6, 0.0);
        let expected = mount_position + mount * bracket_turned(lens, hinge, 45.0);
        assert!((body_position + body * lens - expected).length() < 1e-5);
        let arm = (lens - hinge).length();
        assert!(((body_position + body * lens) - hinge_world).length() - arm < 1e-5);
    }

    #[test]
    fn without_a_hinge_the_whole_fixture_turns_about_its_origin() {
        let fixture = lantern(-30.0, None);
        let (position, orientation) = fixture.placed_by(&[]);
        assert_eq!(position, fixture.position);
        let expected =
            euler_degrees(fixture.rotation_degrees) * Quat::from_rotation_x(-30f32.to_radians());
        assert!(orientation.abs_diff_eq(expected, 1e-6));
    }

    #[test]
    fn a_fixture_sent_before_hinges_existed_turns_about_its_origin() {
        let mut sent = serde_json::to_value(lantern(20.0, Some(Vec3::Y))).unwrap();
        sent.as_object_mut().unwrap().remove("bracket_hinge");
        let read: FixtureInstance = serde_json::from_value(sent).unwrap();
        assert_eq!(read.bracket_hinge, None);
        assert_eq!(read.bracket_degrees, 20.0);
    }

    #[test]
    fn hanging_hardware_is_named_apart_from_the_lamp() {
        for name in [
            "hanging-frame",
            "truss-coupler",
            "truss-coupler-bolt",
            "Hanging-Frame",
        ] {
            assert!(is_mounting_hardware(name), "{name}");
        }
        for name in ["body", "lens", "barn-door-frame", "yoke", "head"] {
            assert!(!is_mounting_hardware(name), "{name}");
        }
    }
}
