//! A fixture's placement as an MVR transform matrix, and back.
//!
//! # ToskLight's convention
//!
//! A patched fixture stores `location` in millimetres and `rotation` in degrees on the desk axes:
//! `x` across the stage, `y` upstage, `z` up. That is right-handed and Z-up, like MVR, so positions
//! need no conversion. The Stage and the visualizer draw a fixture by carrying it into their Y-up
//! world as position `(x, z, −y)` and rotation `(x, z, y)`, composing `Rx·Ry·Rz` there, and then
//! turning the bracket about the fixture's own transverse axis. Brought back onto the desk axes,
//! with column vectors mapping the fixture's own frame into the scene, that is
//!
//! ```text
//! R = Rx(rotation.x) · Rz(rotation.z) · Ry(−rotation.y) · Rx(bracket_angle)
//! ```
//!
//! # MVR's convention
//!
//! MVR 1.6 writes a placement as `{u}{v}{w}{o}`: `u`, `v` and `w` are the fixture's local x, y and z
//! axes in scene coordinates — the columns of `R` — and `o` is its position in millimetres.
//! [`mvr_matrix`] and [`placement_from_mvr`] are exact inverses of each other under that
//! convention, so a rig exchanged with another application hangs the same way in both.

use light_fixture::{FixtureLocation, FixtureVector};

type Matrix3 = [[f64; 3]; 3];

/// Below this the middle rotation is a quarter turn and the outer two share an axis.
const GIMBAL_EPSILON: f64 = 1e-9;

/// The MVR matrix for a fixture at `location` turned by `rotation`, with its bracket set to
/// `bracket_degrees`.
///
/// MVR has no bracket of its own, so the bracket is composed into the orientation: another
/// application opening the file gets the lantern as it hangs, not as it would hang with every
/// clamp set level.
pub fn mvr_matrix(
    location: FixtureLocation,
    rotation: FixtureVector,
    bracket_degrees: f32,
) -> [f64; 12] {
    let r = orientation(rotation, bracket_degrees);
    [
        r[0][0],
        r[1][0],
        r[2][0],
        r[0][1],
        r[1][1],
        r[2][1],
        r[0][2],
        r[1][2],
        r[2][2],
        f64::from(location.x),
        f64::from(location.y),
        f64::from(location.z),
    ]
}

/// The location and rotation an MVR matrix places a fixture at.
///
/// The whole orientation becomes the rotation: MVR carries no bracket, so an imported fixture's
/// bracket is level. Scaled axes are normalised first, since a rotation is all a patch can hold.
pub fn placement_from_mvr(matrix: [f64; 12]) -> (FixtureLocation, FixtureVector) {
    let columns = [
        unit(&matrix[0..3], [1.0, 0.0, 0.0]),
        unit(&matrix[3..6], [0.0, 1.0, 0.0]),
        unit(&matrix[6..9], [0.0, 0.0, 1.0]),
    ];
    let r = |row: usize, column: usize| columns[column][row];
    // R = Rx(a) · Rz(c) · Ry(β): R[0][1] = −sin c, R[0][0] = cos c cos β, R[0][2] = cos c sin β,
    // R[1][1] = cos a cos c, R[2][1] = sin a cos c.
    let c = (-r(0, 1)).clamp(-1.0, 1.0).asin();
    let (a, beta) = if c.cos().abs() > GIMBAL_EPSILON {
        (r(2, 1).atan2(r(1, 1)), r(0, 2).atan2(r(0, 0)))
    } else {
        // Only the sum of the outer turns is fixed; give all of it to x.
        ((-r(1, 2)).atan2(r(2, 2)), 0.0)
    };
    let rotation = FixtureVector {
        x: degrees(a),
        y: degrees(-beta),
        z: degrees(c),
    };
    let location = FixtureLocation {
        x: millimetres(matrix[9]),
        y: millimetres(matrix[10]),
        z: millimetres(matrix[11]),
    };
    (location, rotation)
}

fn orientation(rotation: FixtureVector, bracket_degrees: f32) -> Matrix3 {
    let placement = multiply(
        multiply(
            about_x(f64::from(rotation.x)),
            about_z(f64::from(rotation.z)),
        ),
        about_y(-f64::from(rotation.y)),
    );
    multiply(placement, about_x(f64::from(bracket_degrees)))
}

fn about_x(degrees: f64) -> Matrix3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]]
}

fn about_y(degrees: f64) -> Matrix3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]]
}

fn about_z(degrees: f64) -> Matrix3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]]
}

fn multiply(left: Matrix3, right: Matrix3) -> Matrix3 {
    let mut product = [[0.0; 3]; 3];
    for (row, values) in product.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            *value = (0..3).map(|k| left[row][k] * right[k][column]).sum();
        }
    }
    product
}

/// `axis` scaled to unit length, or `fallback` when it has none.
fn unit(axis: &[f64], fallback: [f64; 3]) -> [f64; 3] {
    let length = axis.iter().map(|value| value * value).sum::<f64>().sqrt();
    if !length.is_finite() || length < GIMBAL_EPSILON {
        return fallback;
    }
    [axis[0] / length, axis[1] / length, axis[2] / length]
}

/// Radians as the degrees a patch stores, without a negative zero.
fn degrees(radians: f64) -> f32 {
    let degrees = radians.to_degrees() as f32;
    if degrees == 0.0 { 0.0 } else { degrees }
}

fn millimetres(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

#[cfg(test)]
#[path = "mvr_transform_tests.rs"]
mod tests;
