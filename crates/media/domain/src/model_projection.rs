//! Where a layer mapped onto a 3D model appears on its output.
//!
//! Every output has the same fixed camera: a perspective with a 40° vertical field of view,
//! standing on the +Z axis and looking down −Z at the origin. It stands exactly far enough back
//! that the unit sphere every imported model is normalized into spans the output height, so a
//! model at scale 1 fills roughly the height of the output the way a Fit layer does.
//!
//! The layer's own controls place the model:
//!
//! - **Position X/Y** move it in the output plane in the same units as a flat layer: `±1` puts its
//!   centre on the left/right or top/bottom edge.
//! - **Scale X/Y** scale it along its own axes; depth scales with their geometric mean.
//! - **Pan, tilt, roll** rotate it in a fixed order, like a moving head: pan turns it about the
//!   vertical axis first; tilt then tips it about its (panned) horizontal axis; roll finally spins
//!   it about its own facing axis. At pan and tilt 0 that facing axis is the viewing axis, so roll
//!   looks exactly like the flat layer Rotation. Positive pan turns the front to the right,
//!   positive tilt tips the front up, positive roll turns clockwise as seen on the output.
//!
//! Matrices are column-major (`matrix[column][row]`), the layout WGSL's `mat4x4<f32>` expects.

use crate::geometry::Size;
use crate::layer::LayerState;

pub type Matrix4 = [[f32; 4]; 4];

/// The camera's vertical field of view.
pub const MODEL_VERTICAL_FOV_DEGREES: f32 = 40.0;
const NEAR: f32 = 0.05;
const FAR: f32 = 100.0;

/// How far the camera stands from the origin: where the unit sphere exactly spans the height.
pub fn camera_distance() -> f32 {
    1.0 / (MODEL_VERTICAL_FOV_DEGREES.to_radians() * 0.5).tan()
}

pub const IDENTITY: Matrix4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub fn multiply(a: &Matrix4, b: &Matrix4) -> Matrix4 {
    let mut out = [[0.0; 4]; 4];
    for (column, out_column) in out.iter_mut().enumerate() {
        for (row, value) in out_column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

pub fn transform(matrix: &Matrix4, point: [f32; 3]) -> [f32; 4] {
    let [x, y, z] = point;
    [0, 1, 2, 3]
        .map(|row| matrix[0][row] * x + matrix[1][row] * y + matrix[2][row] * z + matrix[3][row])
}

fn translation(x: f32, y: f32, z: f32) -> Matrix4 {
    let mut matrix = IDENTITY;
    matrix[3] = [x, y, z, 1.0];
    matrix
}

fn scaling(x: f32, y: f32, z: f32) -> Matrix4 {
    [
        [x, 0.0, 0.0, 0.0],
        [0.0, y, 0.0, 0.0],
        [0.0, 0.0, z, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rotation_x(degrees: f32) -> Matrix4 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cos, sin, 0.0],
        [0.0, -sin, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rotation_y(degrees: f32) -> Matrix4 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [
        [cos, 0.0, -sin, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [sin, 0.0, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rotation_z(degrees: f32) -> Matrix4 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [
        [cos, sin, 0.0, 0.0],
        [-sin, cos, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Pan, then tilt, then roll: `Ry(pan) · Rx(−tilt) · Rz(−roll)`, applied to a model-space point.
pub fn model_rotation(pan: f32, tilt: f32, roll: f32) -> Matrix4 {
    multiply(
        &rotation_y(pan),
        &multiply(&rotation_x(-tilt), &rotation_z(-roll)),
    )
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// Model space to world space for one layer.
pub fn model_matrix(layer: &LayerState, output: Size) -> Matrix4 {
    let aspect = aspect(output);
    let scale_x = finite_or(layer.scale_x, 1.0);
    let scale_y = finite_or(layer.scale_y, 1.0);
    let scale_z = (scale_x * scale_y).abs().sqrt();
    let rotation = model_rotation(
        finite_or(layer.model.pan, 0.0),
        finite_or(layer.model.tilt, 0.0),
        finite_or(layer.rotation, 0.0),
    );
    multiply(
        &translation(
            finite_or(layer.position_x, 0.0) * aspect,
            -finite_or(layer.position_y, 0.0),
            0.0,
        ),
        &multiply(&rotation, &scaling(scale_x, scale_y, scale_z)),
    )
}

fn aspect(output: Size) -> f32 {
    if output.is_empty() {
        1.0
    } else {
        output.width as f32 / output.height as f32
    }
}

/// World space to clip space for an output: the fixed camera and its perspective, with wgpu's
/// `0..1` depth range.
pub fn view_projection(output: Size) -> Matrix4 {
    let focal = camera_distance();
    let aspect = aspect(output);
    let depth = FAR / (NEAR - FAR);
    let projection = [
        [focal / aspect, 0.0, 0.0, 0.0],
        [0.0, focal, 0.0, 0.0],
        [0.0, 0.0, depth, -1.0],
        [0.0, 0.0, NEAR * depth, 0.0],
    ];
    multiply(&projection, &translation(0.0, 0.0, -camera_distance()))
}

/// Model space to clip space for one layer on one output.
pub fn model_view_projection(layer: &LayerState, output: Size) -> Matrix4 {
    multiply(&view_projection(output), &model_matrix(layer, output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::ModelMapping;

    const OUTPUT: Size = Size::new(1920, 1080);

    fn ndc(layer: &LayerState, point: [f32; 3]) -> [f32; 3] {
        let clip = transform(&model_view_projection(layer, OUTPUT), point);
        [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]]
    }

    fn close(actual: f32, expected: f32) -> bool {
        (actual - expected).abs() < 1e-4
    }

    fn mapped(pan: f32, tilt: f32, roll: f32) -> LayerState {
        LayerState {
            rotation: roll,
            model: ModelMapping {
                model: 1,
                pan,
                tilt,
            },
            ..Default::default()
        }
    }

    #[test]
    fn the_unit_sphere_spans_the_output_height_at_scale_one() {
        let layer = mapped(0.0, 0.0, 0.0);
        let top = ndc(&layer, [0.0, 1.0, 0.0]);
        assert!(close(top[0], 0.0) && close(top[1], 1.0), "{top:?}");
        let right = ndc(&layer, [1920.0 / 1080.0, 0.0, 0.0]);
        assert!(close(right[0], 1.0), "{right:?}");
        assert!((0.0..1.0).contains(&top[2]), "inside the depth range");
    }

    #[test]
    fn position_uses_the_flat_layer_units() {
        let layer = LayerState {
            position_x: 1.0,
            position_y: 1.0,
            ..mapped(0.0, 0.0, 0.0)
        };
        let centre = ndc(&layer, [0.0, 0.0, 0.0]);
        assert!(close(centre[0], 1.0), "right edge: {centre:?}");
        assert!(close(centre[1], -1.0), "positive Y is down: {centre:?}");
    }

    #[test]
    fn scale_scales_about_the_model_centre() {
        let layer = LayerState {
            scale_x: 0.5,
            scale_y: 0.5,
            ..mapped(0.0, 0.0, 0.0)
        };
        assert!(close(ndc(&layer, [0.0, 1.0, 0.0])[1], 0.5));
    }

    #[test]
    fn roll_turns_clockwise_like_the_flat_rotation() {
        let top = ndc(&mapped(0.0, 0.0, 90.0), [0.0, 0.5, 0.0]);
        assert!(
            top[0] > 0.2 && close(top[1], 0.0),
            "top moved right: {top:?}"
        );
    }

    #[test]
    fn pan_turns_the_front_right_and_quarter_pan_is_edge_on() {
        let rotation = model_rotation(90.0, 0.0, 0.0);
        let front = transform(&rotation, [0.0, 0.0, 1.0]);
        assert!(close(front[0], 1.0) && close(front[2], 0.0), "{front:?}");
        // A point on the model's right edge now lies on the viewing axis.
        let edge = ndc(&mapped(90.0, 0.0, 0.0), [0.5, 0.0, 0.0]);
        assert!(close(edge[0], 0.0), "{edge:?}");
    }

    #[test]
    fn tilt_tips_the_front_up() {
        let front = transform(&model_rotation(0.0, 90.0, 0.0), [0.0, 0.0, 1.0]);
        assert!(close(front[1], 1.0), "{front:?}");
    }

    #[test]
    fn the_order_is_pan_then_tilt_then_roll_like_a_moving_head() {
        // Pan 90 faces the front right. Tilt then tips it about its own horizontal axis, so the
        // front rises instead of the model spinning about the screen's horizontal axis.
        let front = transform(&model_rotation(90.0, 45.0, 0.0), [0.0, 0.0, 1.0]);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            close(front[0], half) && close(front[1], half) && close(front[2], 0.0),
            "{front:?}"
        );
        // Roll last spins about the model's own facing axis, leaving that axis where it was.
        let rolled = transform(&model_rotation(90.0, 45.0, 30.0), [0.0, 0.0, 1.0]);
        assert!((0..3).all(|axis| close(rolled[axis], front[axis])));
    }
}
