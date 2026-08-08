//! Layer geometry.
//!
//! Turning a layer's state and its source's dimensions into a placed, rotated quad is pure
//! arithmetic, so it lives here rather than in the renderer. The renderer consumes the result;
//! the tests do not need a GPU to check it.
//!
//! Output coordinates are in pixels with the origin at the top left and `y` increasing downward,
//! which is what the DMX position formula is written against.

use serde::{Deserialize, Serialize};

use crate::color::FlipMirror;
use crate::layer::{LayerState, ScalingMode};

/// A width and height in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Whether this size can be divided by. A zero-sized source or output has no meaningful
    /// scale factor, and asking for one would produce infinities.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// A point in output pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Where a layer's quad sits on the output.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerTransform {
    /// The quad's center, in output pixels.
    pub center: Point,
    /// The quad's unrotated width and height, in output pixels.
    pub size: (f32, f32),
    /// Rotation around [`LayerTransform::center`], clockwise in this coordinate system.
    pub rotation_degrees: f32,
}

impl LayerTransform {
    /// The four corners in output pixels, clockwise from the top left of the unrotated quad.
    ///
    /// Corners are the honest thing to assert in a test: they fold the scaling mode, the user
    /// scale, the position, and the rotation into one observable answer.
    pub fn corners(&self) -> [Point; 4] {
        let (half_width, half_height) = (self.size.0 / 2.0, self.size.1 / 2.0);
        let radians = self.rotation_degrees.to_radians();
        let (sin, cos) = radians.sin_cos();

        [
            (-half_width, -half_height),
            (half_width, -half_height),
            (half_width, half_height),
            (-half_width, half_height),
        ]
        .map(|(x, y)| {
            Point::new(
                self.center.x + x * cos - y * sin,
                self.center.y + x * sin + y * cos,
            )
        })
    }
}

/// The uniform or per-axis factor a scaling mode applies before the operator's own scale.
///
/// Fit and Fill are uniform, so a source never distorts unless the operator asks it to.
pub fn scaling_mode_factor(mode: ScalingMode, source: Size, output: Size) -> (f32, f32) {
    if source.is_empty() || output.is_empty() {
        return (0.0, 0.0);
    }
    let horizontal = output.width as f32 / source.width as f32;
    let vertical = output.height as f32 / source.height as f32;

    match mode {
        ScalingMode::Fit => {
            let uniform = horizontal.min(vertical);
            (uniform, uniform)
        }
        ScalingMode::Fill => {
            let uniform = horizontal.max(vertical);
            (uniform, uniform)
        }
        ScalingMode::Original => (1.0, 1.0),
        ScalingMode::Stretch => (horizontal, vertical),
    }
}

/// Places a layer on an output.
pub fn layer_transform(layer: &LayerState, source: Size, output: Size) -> LayerTransform {
    let (fit_x, fit_y) = scaling_mode_factor(layer.scaling_mode, source, output);

    let width = source.width as f32 * fit_x * layer.scale_x;
    let height = source.height as f32 * fit_y * layer.scale_y;

    // The documented DMX position formula. `0.0` is centered, `±1.0` puts the layer's center on
    // an edge, and `±2.0` moves it a further half-screen outside.
    let half_output_width = output.width as f32 / 2.0;
    let half_output_height = output.height as f32 / 2.0;

    LayerTransform {
        center: Point::new(
            half_output_width + layer.position_x * half_output_width,
            half_output_height + layer.position_y * half_output_height,
        ),
        size: (width, height),
        rotation_degrees: layer.rotation,
    }
}

/// The master flip, applied to the output coordinate system after the composite is complete.
///
/// Returned as a per-axis sign so the renderer can fold it into one final transform rather than
/// re-drawing anything.
pub const fn flip_signs(flip: FlipMirror) -> (f32, f32) {
    (
        if flip.flips_horizontally() { -1.0 } else { 1.0 },
        if flip.flips_vertically() { -1.0 } else { 1.0 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: Size = Size::new(1920, 1080);

    fn layer() -> LayerState {
        LayerState::default()
    }

    fn close(actual: f32, expected: f32) -> bool {
        (actual - expected).abs() < 1e-3
    }

    #[test]
    fn fit_shows_the_whole_source_inside_the_output() {
        // A square source in a 16:9 output is limited by height.
        let (x, y) = scaling_mode_factor(ScalingMode::Fit, Size::new(1000, 1000), OUTPUT);
        assert!(close(x, 1.08) && close(y, 1.08));
        assert_eq!(x, y, "Fit is uniform");
    }

    #[test]
    fn fill_covers_the_output_and_crops_the_overflow() {
        let (x, y) = scaling_mode_factor(ScalingMode::Fill, Size::new(1000, 1000), OUTPUT);
        assert!(close(x, 1.92) && close(y, 1.92));
        assert_eq!(x, y, "Fill is uniform");
    }

    #[test]
    fn original_maps_one_source_pixel_to_one_output_pixel() {
        assert_eq!(
            scaling_mode_factor(ScalingMode::Original, Size::new(640, 480), OUTPUT),
            (1.0, 1.0)
        );
    }

    #[test]
    fn stretch_is_the_only_mode_that_distorts() {
        let (x, y) = scaling_mode_factor(ScalingMode::Stretch, Size::new(1000, 1000), OUTPUT);
        assert!(close(x, 1.92) && close(y, 1.08));
        assert_ne!(x, y);
    }

    #[test]
    fn a_zero_sized_source_or_output_produces_no_scale_rather_than_an_infinity() {
        for (source, output) in [
            (Size::new(0, 100), OUTPUT),
            (Size::new(100, 0), OUTPUT),
            (Size::new(100, 100), Size::new(0, 0)),
        ] {
            let (x, y) = scaling_mode_factor(ScalingMode::Fit, source, output);
            assert!(x.is_finite() && y.is_finite());
            assert_eq!((x, y), (0.0, 0.0));
        }
    }

    #[test]
    fn a_neutral_layer_fills_the_output_when_the_source_matches_it() {
        let transform = layer_transform(&layer(), Size::new(1920, 1080), OUTPUT);
        assert_eq!(transform.center, Point::new(960.0, 540.0));
        assert_eq!(transform.size, (1920.0, 1080.0));
        assert_eq!(transform.rotation_degrees, 0.0);
    }

    #[test]
    fn the_user_scale_multiplies_the_scaling_mode() {
        let scaled = LayerState {
            scale_x: 0.5,
            scale_y: 2.0,
            ..layer()
        };
        let transform = layer_transform(&scaled, Size::new(1920, 1080), OUTPUT);
        assert_eq!(transform.size, (960.0, 2160.0));
    }

    #[test]
    fn each_axis_scales_independently() {
        let wide = LayerState {
            scale_x: 3.0,
            ..layer()
        };
        let transform = layer_transform(&wide, Size::new(100, 100), Size::new(100, 100));
        assert_eq!(transform.size, (300.0, 100.0));
    }

    #[test]
    fn position_one_puts_the_layer_center_on_an_edge() {
        let right = LayerState {
            position_x: 1.0,
            ..layer()
        };
        assert_eq!(
            layer_transform(&right, OUTPUT, OUTPUT).center,
            Point::new(1920.0, 540.0)
        );

        let left = LayerState {
            position_x: -1.0,
            ..layer()
        };
        assert_eq!(
            layer_transform(&left, OUTPUT, OUTPUT).center,
            Point::new(0.0, 540.0)
        );

        let down = LayerState {
            position_y: 1.0,
            ..layer()
        };
        assert_eq!(
            layer_transform(&down, OUTPUT, OUTPUT).center,
            Point::new(960.0, 1080.0)
        );
    }

    #[test]
    fn position_two_moves_a_further_half_screen_outside() {
        let far = LayerState {
            position_x: 2.0,
            position_y: -2.0,
            ..layer()
        };
        let transform = layer_transform(&far, OUTPUT, OUTPUT);
        assert_eq!(transform.center, Point::new(2880.0, -540.0));
    }

    #[test]
    fn the_corners_of_an_unrotated_quad_are_its_bounds() {
        let transform = layer_transform(&layer(), OUTPUT, OUTPUT);
        let [top_left, top_right, bottom_right, bottom_left] = transform.corners();
        assert_eq!(top_left, Point::new(0.0, 0.0));
        assert_eq!(top_right, Point::new(1920.0, 0.0));
        assert_eq!(bottom_right, Point::new(1920.0, 1080.0));
        assert_eq!(bottom_left, Point::new(0.0, 1080.0));
    }

    #[test]
    fn rotation_turns_the_quad_around_its_own_center() {
        let square = Size::new(100, 100);
        let output = Size::new(100, 100);
        let turned = LayerState {
            rotation: 90.0,
            ..layer()
        };
        let corners = layer_transform(&turned, square, output).corners();

        // A quarter turn sends the top-left corner to the top-right position.
        assert!(close(corners[0].x, 100.0) && close(corners[0].y, 0.0));
        assert!(close(corners[1].x, 100.0) && close(corners[1].y, 100.0));
        assert!(close(corners[2].x, 0.0) && close(corners[2].y, 100.0));
        assert!(close(corners[3].x, 0.0) && close(corners[3].y, 0.0));
    }

    #[test]
    fn a_full_turn_each_way_returns_the_quad_to_where_it_started() {
        let square = Size::new(100, 100);
        let rest = layer_transform(&layer(), square, square).corners();
        for rotation in [-360.0, 360.0] {
            let turned = LayerState {
                rotation,
                ..layer()
            };
            for (actual, expected) in layer_transform(&turned, square, square)
                .corners()
                .iter()
                .zip(&rest)
            {
                assert!(
                    close(actual.x, expected.x) && close(actual.y, expected.y),
                    "{rotation}"
                );
            }
        }
    }

    #[test]
    fn rotation_happens_after_positioning_so_it_never_moves_the_center() {
        let moved = LayerState {
            position_x: 0.5,
            rotation: 37.0,
            ..layer()
        };
        let transform = layer_transform(&moved, OUTPUT, OUTPUT);
        assert_eq!(transform.center, Point::new(1440.0, 540.0));

        let average_x = transform
            .corners()
            .iter()
            .map(|corner| corner.x)
            .sum::<f32>()
            / 4.0;
        let average_y = transform
            .corners()
            .iter()
            .map(|corner| corner.y)
            .sum::<f32>()
            / 4.0;
        assert!(close(average_x, 1440.0) && close(average_y, 540.0));
    }

    #[test]
    fn the_master_flip_becomes_a_sign_on_each_axis() {
        assert_eq!(flip_signs(FlipMirror::None), (1.0, 1.0));
        assert_eq!(flip_signs(FlipMirror::Horizontal), (-1.0, 1.0));
        assert_eq!(flip_signs(FlipMirror::Vertical), (1.0, -1.0));
        assert_eq!(flip_signs(FlipMirror::Both), (-1.0, -1.0));
    }
}
