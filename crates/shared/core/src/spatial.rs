//! Renderer-independent rigid transforms. Coordinates passed here are metres;
//! column vectors and composition `a.compose(b)` apply b first, then a.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RigidTransform {
    rotation: [[f64; 3]; 3],
    translation: [f64; 3],
}
impl RigidTransform {
    pub const IDENTITY: Self = Self {
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: [0.0; 3],
    };
    pub fn translation(value: [f64; 3]) -> Option<Self> {
        value.iter().all(|v| v.is_finite()).then_some(Self {
            translation: value,
            ..Self::IDENTITY
        })
    }
    pub fn axis_angle(axis: [f64; 3], degrees: f64) -> Option<Self> {
        if !degrees.is_finite() || !axis.iter().all(|v| v.is_finite()) {
            return None;
        }
        let length = axis[0].hypot(axis[1]).hypot(axis[2]);
        if length < 1e-12 || !length.is_finite() {
            return None;
        }
        let [x, y, z] = axis.map(|v| v / length);
        let (s, c) = degrees.to_radians().sin_cos();
        let t = 1.0 - c;
        Some(Self {
            rotation: [
                [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
                [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
                [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
            ],
            ..Self::IDENTITY
        })
    }
    /// Intrinsic XYZ, written Rx * Ry * Rz, matching the authored neutral geometry.
    pub fn euler_xyz(degrees: [f64; 3]) -> Option<Self> {
        Some(
            Self::axis_angle([1.0, 0.0, 0.0], degrees[0])?
                .compose(Self::axis_angle([0.0, 1.0, 0.0], degrees[1])?)
                .compose(Self::axis_angle([0.0, 0.0, 1.0], degrees[2])?),
        )
    }
    /// Canonical XYZ degrees for this rotation. At gimbal lock Z is zero;
    /// callers must compare reconstructed rotations rather than Euler triples.
    pub fn euler_xyz_degrees(self) -> [f64; 3] {
        let r = self.rotation;
        let cos_y = r[0][0].hypot(r[0][1]);
        let y = r[0][2].atan2(cos_y);
        let (x, z) = if cos_y > 1e-9 {
            ((-r[1][2]).atan2(r[2][2]), (-r[0][1]).atan2(r[0][0]))
        } else {
            (r[2][1].atan2(r[1][1]), 0.0)
        };
        [x.to_degrees(), y.to_degrees(), z.to_degrees()]
    }
    pub fn direction(self, v: [f64; 3]) -> [f64; 3] {
        self.rotation
            .map(|r| r[0] * v[0] + r[1] * v[1] + r[2] * v[2])
    }
    pub fn point(self, v: [f64; 3]) -> [f64; 3] {
        let r = self.direction(v);
        std::array::from_fn(|i| r[i] + self.translation[i])
    }
    pub fn compose(self, rhs: Self) -> Self {
        let rotation = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                (0..3)
                    .map(|k| self.rotation[i][k] * rhs.rotation[k][j])
                    .sum()
            })
        });
        Self {
            rotation,
            translation: self.point(rhs.translation),
        }
    }
    pub fn inverse(self) -> Self {
        let rotation = std::array::from_fn(|i| std::array::from_fn(|j| self.rotation[j][i]));
        let inverse = Self {
            rotation,
            ..Self::IDENTITY
        };
        Self {
            translation: inverse.direction(self.translation.map(|v| -v)),
            ..inverse
        }
    }
    pub fn about_pivot(rotation: Self, pivot: [f64; 3]) -> Option<Self> {
        Some(
            Self::translation(pivot)?
                .compose(rotation)
                .compose(Self::translation(pivot.map(|v| -v))?),
        )
    }
    /// Desk X/Y/Z-up to fixture and renderer X/Y-up/-Z coordinates.
    pub const DESK_TO_PROFILE: Self = Self {
        rotation: [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
        translation: [0.0; 3],
    };
    pub fn desk_pose_to_profile(self) -> Self {
        Self::DESK_TO_PROFILE
            .compose(self)
            .compose(Self::DESK_TO_PROFILE.inverse())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn near(actual: [f64; 3], expected: [f64; 3]) {
        for i in 0..3 {
            assert!(
                (actual[i] - expected[i]).abs() < 1e-10,
                "{actual:?} != {expected:?}"
            )
        }
    }
    #[test]
    fn basis_conjugation_preserves_rotation_direction() {
        let desk = RigidTransform::euler_xyz([0.0, 90.0, 0.0]).unwrap();
        near(
            desk.desk_pose_to_profile().direction([1.0, 0.0, 0.0]),
            [0.0, -1.0, 0.0],
        );
        near(
            RigidTransform::DESK_TO_PROFILE.direction(desk.direction([1.0, 0.0, 0.0])),
            [0.0, -1.0, 0.0],
        );
    }
    #[test]
    fn moving_reference_composes_mount_rotation_without_euler_addition() {
        let t = |v| RigidTransform::translation(v).unwrap();
        let r = |v| RigidTransform::euler_xyz(v).unwrap();
        let point = t([0.0, 0.0, 1.0])
            .compose(RigidTransform::about_pivot(r([0.0, 0.0, 90.0]), [1.0, 2.0, 3.0]).unwrap());
        let mount = point.compose(t([3.0, 2.0, 3.0]).compose(r([90.0, 0.0, 0.0])));
        near(mount.point([0.0; 3]), [1.0, 4.0, 4.0]);
        near(mount.direction([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        near(
            mount.inverse().point(mount.point([0.2, 0.3, -0.4])),
            [0.2, 0.3, -0.4],
        );
    }
    #[test]
    fn bracket_hinge_moves_the_lens_and_keeps_pivot_fixed() {
        let pivot = [0.0, -0.35, 0.0];
        let lens = [0.0, -0.6, 0.1];
        let hinge = RigidTransform::about_pivot(
            RigidTransform::axis_angle([1.0, 0.0, 0.0], 90.0).unwrap(),
            pivot,
        )
        .unwrap();
        near(hinge.point(pivot), pivot);
        near(hinge.point(lens), [0.0, -0.45, -0.25]);
        near(hinge.direction([0.0, -1.0, 0.0]), [0.0, 0.0, -1.0]);
    }
    #[test]
    fn xyz_extraction_preserves_compound_and_gimbal_rotations() {
        for angles in [
            [24., 37., -52.],
            [34., 90., 75.],
            [34., -90., 75.],
            [165., 110., -143.],
        ] {
            let expected = RigidTransform::euler_xyz(angles).unwrap();
            let actual = RigidTransform::euler_xyz(expected.euler_xyz_degrees()).unwrap();
            for v in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
                near(actual.direction(v), expected.direction(v));
            }
        }
    }
    #[test]
    fn composed_quarter_turn_does_not_extract_roundoff_as_rotation() {
        let expected = RigidTransform::euler_xyz([10., 35., 0.])
            .unwrap()
            .compose(RigidTransform::euler_xyz([0., 55., 80.]).unwrap());
        let actual = RigidTransform::euler_xyz(expected.euler_xyz_degrees()).unwrap();
        for axis in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
            near(actual.direction(axis), expected.direction(axis));
        }
    }
    #[test]
    fn invalid_axes_and_nonfinite_poses_are_rejected() {
        assert!(RigidTransform::axis_angle([0.0; 3], 1.0).is_none());
        assert!(RigidTransform::euler_xyz([0.0, f64::NAN, 0.0]).is_none());
        assert!(RigidTransform::translation([f64::INFINITY, 0.0, 0.0]).is_none());
    }
}
