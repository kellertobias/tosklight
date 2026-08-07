//! Where the helper is allowed to draw.
//!
//! For the desk's own window the answer is "all of it". For the Stage pane it is a rectangle the
//! web layout decided, and the helper must draw inside it and nowhere else — the surrounding
//! chrome belongs to the webview, and a renderer that overshot would paint over the sheet.
//!
//! The web side owns the geometry because the pane is an element in a layout; the helper is told.
//! Both sides therefore need one definition of what a pane rectangle means, and the conversion
//! from the points a layout works in to the pixels a surface works in has to happen once, in a
//! place that can be tested without a GPU or a browser.
//!
//! This is the contract the embedded-pane experiment established, lifted out of it so the desk and
//! the helper share it rather than each having their own idea.

use serde::{Deserialize, Serialize};

/// The pane in logical points, from the top-left of the window, as a web layout reports it.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PaneRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The same rectangle in physical pixels, clamped to a surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PanePixels {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PaneRect {
    /// The whole of a surface, which is what a window that is nothing but the renderer gets.
    pub fn full(width: f32, height: f32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    /// Convert to the pixels a scissor takes, clamped to the surface.
    ///
    /// Returns `None` when there is nothing to draw — a collapsed pane, a pane scrolled entirely
    /// off the surface, or a layout that has not run yet. That is not an error: the caller skips
    /// the frame, which is what keeps a half-laid-out window from flashing.
    ///
    /// Clamping rather than trusting is the point. The rectangle comes from the other side of an
    /// IPC boundary, and a scissor past the end of a surface is a validation failure that takes
    /// the renderer down — the far side must not be able to do that by reporting a stale layout
    /// one frame after a resize.
    pub fn to_pixels(
        self,
        scale: f32,
        surface_width: u32,
        surface_height: u32,
    ) -> Option<PanePixels> {
        if !self.width.is_finite() || !self.height.is_finite() {
            return None;
        }
        if !self.x.is_finite() || !self.y.is_finite() {
            return None;
        }
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let x = (self.x * scale).round().max(0.0) as u32;
        let y = (self.y * scale).round().max(0.0) as u32;
        if x >= surface_width || y >= surface_height {
            return None;
        }
        let width = (self.width * scale).round().max(0.0) as u32;
        let height = (self.height * scale).round().max(0.0) as u32;
        let width = width.min(surface_width - x);
        let height = height.min(surface_height - y);
        if width == 0 || height == 0 {
            return None;
        }
        Some(PanePixels {
            x,
            y,
            width,
            height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_scales_from_points_to_pixels() {
        let pane = PaneRect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };
        assert_eq!(
            pane.to_pixels(2.0, 1000, 1000),
            Some(PanePixels {
                x: 20,
                y: 40,
                width: 200,
                height: 100
            })
        );
    }

    #[test]
    fn a_full_surface_pane_is_the_whole_surface() {
        let pane = PaneRect::full(800.0, 600.0);
        assert_eq!(
            pane.to_pixels(1.0, 800, 600),
            Some(PanePixels {
                x: 0,
                y: 0,
                width: 800,
                height: 600
            })
        );
    }

    /// The rectangle crosses an IPC boundary, and a scissor past the end of a surface takes the
    /// renderer down. A layout reported one frame after a resize must not be able to do that.
    #[test]
    fn a_pane_larger_than_the_surface_is_clamped_rather_than_trusted() {
        let pane = PaneRect {
            x: 100.0,
            y: 100.0,
            width: 5_000.0,
            height: 5_000.0,
        };
        let pixels = pane.to_pixels(1.0, 800, 600).expect("something to draw");
        assert_eq!(pixels.x + pixels.width, 800);
        assert_eq!(pixels.y + pixels.height, 600);
    }

    #[test]
    fn a_collapsed_pane_has_nothing_to_draw() {
        let pane = PaneRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 100.0,
        };
        assert_eq!(pane.to_pixels(1.0, 800, 600), None);
    }

    /// A pane scrolled entirely off the surface is not an error either — the frame is skipped.
    #[test]
    fn a_pane_beyond_the_surface_has_nothing_to_draw() {
        let pane = PaneRect {
            x: 900.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        assert_eq!(pane.to_pixels(1.0, 800, 600), None);
    }

    /// Before the web layout has run there is no rectangle. Drawing the whole surface then would
    /// flash the renderer over chrome that is about to appear.
    #[test]
    fn an_unreported_pane_draws_nothing() {
        assert_eq!(PaneRect::default().to_pixels(1.0, 800, 600), None);
    }

    /// The far side is not trusted to send a number at all.
    #[test]
    fn a_pane_that_is_not_a_rectangle_draws_nothing() {
        for pane in [
            PaneRect {
                x: f32::NAN,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            PaneRect {
                x: 0.0,
                y: 0.0,
                width: f32::INFINITY,
                height: 10.0,
            },
        ] {
            assert_eq!(pane.to_pixels(1.0, 800, 600), None, "{pane:?}");
        }
    }

    /// A nonsense scale factor falls back to one rather than collapsing the pane to nothing.
    #[test]
    fn an_impossible_scale_is_treated_as_one() {
        let pane = PaneRect::full(100.0, 100.0);
        assert_eq!(pane.to_pixels(0.0, 800, 600), pane.to_pixels(1.0, 800, 600));
    }
}
