//! Aiming a moving light at a point in the rig.
//!
//! `Fixture 1 AT Fixture 5` asks the desk a geometric question: what pan and tilt put this
//! lantern's beam on that object? The answer depends on where both of them actually are, which is
//! not always where the patch put them — a fixture slaved to a 3D Point moves when the point does,
//! and the aim has to follow the object rather than the address.
//!
//! Deliberately free of any renderer type. The desk answers this while programming, long before
//! anything is drawn, and a visualizer being open is not a precondition for pointing a light.

/// A point in desk world space, in metres.
pub type Point = [f32; 3];

/// Where a fixture is and which way it is hung.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mount {
    pub position: Point,
    /// Mounting rotation in degrees about the world axes, applied `Rx * Ry * Rz`.
    pub rotation_degrees: [f32; 3],
}

/// Pan and tilt in degrees that aim `mount` at `target`.
///
/// Returns `None` when the two are in the same place, where no direction exists and any answer
/// would be invented.
///
/// The convention matches the renderer's: a head rests aiming along its own `-Y`, pan turns it
/// about `Y` and tilt about `X`, and the mounting rotation is applied outside both. So the target
/// is first brought into the fixture's own frame and the angles are read off there, which is what
/// makes a lantern hung upside-down on a truss aim correctly rather than mirrored.
pub fn pan_tilt_towards(mount: Mount, target: Point) -> Option<(f32, f32)> {
    let delta = [
        target[0] - mount.position[0],
        target[1] - mount.position[1],
        target[2] - mount.position[2],
    ];
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if length < 1e-6 {
        return None;
    }
    let direction = [delta[0] / length, delta[1] / length, delta[2] / length];
    let local = unrotate(direction, mount.rotation_degrees);
    // From `Ry(pan) * Rx(tilt) * (0, -1, 0)`, the local aim is
    // `(-sin(tilt)sin(pan), -cos(tilt), -sin(tilt)cos(pan))`.
    let tilt = (-local[1]).clamp(-1.0, 1.0).acos();
    // Both components carry the same `-sin(tilt)` factor, so they are negated before the
    // arctangent: taking them as they are lands the pan half a turn out.
    let pan = (-local[0]).atan2(-local[2]);
    Some((pan.to_degrees(), tilt.to_degrees()))
}

/// Bring a world direction into the fixture's own frame by undoing `Rx * Ry * Rz`.
fn unrotate(direction: [f32; 3], rotation_degrees: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = rotation_degrees;
    let after_x = rotate_x(direction, -x.to_radians());
    let after_y = rotate_y(after_x, -y.to_radians());
    rotate_z(after_y, -z.to_radians())
}

fn rotate_x(v: [f32; 3], angle: f32) -> [f32; 3] {
    let (sin, cos) = angle.sin_cos();
    [v[0], v[1] * cos - v[2] * sin, v[1] * sin + v[2] * cos]
}

fn rotate_y(v: [f32; 3], angle: f32) -> [f32; 3] {
    let (sin, cos) = angle.sin_cos();
    [v[0] * cos + v[2] * sin, v[1], -v[0] * sin + v[2] * cos]
}

fn rotate_z(v: [f32; 3], angle: f32) -> [f32; 3] {
    let (sin, cos) = angle.sin_cos();
    [v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos, v[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hung(position: Point) -> Mount {
        Mount {
            position,
            rotation_degrees: [0.0; 3],
        }
    }

    #[test]
    fn a_lantern_aims_straight_down_at_what_is_directly_beneath_it() {
        let (pan, tilt) = pan_tilt_towards(hung([0.0, 6.0, 0.0]), [0.0, 0.0, 0.0]).unwrap();
        // Straight down is the rest position: no tilt, and pan does not matter.
        assert!(tilt.abs() < 1e-3, "tilt was {tilt}");
        assert!(pan.is_finite());
    }

    #[test]
    fn a_lantern_tilts_a_quarter_turn_to_look_level() {
        let (_, tilt) = pan_tilt_towards(hung([0.0, 6.0, 0.0]), [0.0, 6.0, -10.0]).unwrap();
        assert!((tilt - 90.0).abs() < 1e-3, "tilt was {tilt}");
    }

    #[test]
    fn pan_follows_the_target_around_the_fixture() {
        let level_upstage = pan_tilt_towards(hung([0.0, 6.0, 0.0]), [0.0, 6.0, -10.0])
            .unwrap()
            .0;
        let level_stage_right = pan_tilt_towards(hung([0.0, 6.0, 0.0]), [10.0, 6.0, 0.0])
            .unwrap()
            .0;
        // Ninety degrees apart, whichever way round the convention runs.
        let difference = (level_stage_right - level_upstage).abs();
        assert!((difference - 90.0).abs() < 1e-3, "difference {difference}");
    }

    /// Rebuild the world aim from the angles, the way the renderer does: `mount * Ry(pan) *
    /// Rx(tilt)` applied to the rest direction `-Y`.
    fn aim_direction(mount: Mount, pan: f32, tilt: f32) -> [f32; 3] {
        let local = rotate_y(
            rotate_x([0.0, -1.0, 0.0], tilt.to_radians()),
            pan.to_radians(),
        );
        let [rx, ry, rz] = mount.rotation_degrees;
        rotate_x(
            rotate_y(rotate_z(local, rz.to_radians()), ry.to_radians()),
            rx.to_radians(),
        )
    }

    fn assert_hits(mount: Mount, target: Point) {
        let (pan, tilt) = pan_tilt_towards(mount, target).expect("an aim exists");
        let aim = aim_direction(mount, pan, tilt);
        let delta = [
            target[0] - mount.position[0],
            target[1] - mount.position[1],
            target[2] - mount.position[2],
        ];
        let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        let wanted = [delta[0] / length, delta[1] / length, delta[2] / length];
        for axis in 0..3 {
            assert!(
                (aim[axis] - wanted[axis]).abs() < 1e-3,
                "aimed {aim:?} at {wanted:?} from {mount:?}"
            );
        }
    }

    #[test]
    fn the_angles_actually_put_the_beam_on_the_target() {
        // However the lantern is hung, the angles it is given have to reach the same object. The
        // angles themselves differ — an inverted head tilts the other way round — so the property
        // worth asserting is the aim, not the numbers.
        for rotation in [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 180.0],
            [0.0, 90.0, 0.0],
            [15.0, -40.0, 25.0],
        ] {
            for target in [
                [2.0, 0.0, -3.0],
                [-4.0, 1.2, 5.0],
                [0.0, 0.0, 0.0],
                [7.5, 6.0, 0.25],
            ] {
                assert_hits(
                    Mount {
                        position: [0.0, 6.0, 0.0],
                        rotation_degrees: rotation,
                    },
                    target,
                );
            }
        }
    }

    #[test]
    fn a_fixture_cannot_aim_at_itself() {
        assert!(pan_tilt_towards(hung([1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]).is_none());
    }
}
