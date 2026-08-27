//! Display regions: which part of the canvas one screen shows, and which way up.
//!
//! One Media Server canvas often feeds several screens at once — a slice of it turned on its side
//! for a portrait wall, and two more slices thrown by projectors either side of it. A region is
//! that arrangement: a rectangle of the canvas, a place to put it on the screen, and a rotation
//! applied to that screen alone.

use crate::pixel_map::CanvasPoint;
use serde::{Deserialize, Serialize};

/// A rectangle in canvas fractions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasRect {
    pub start: CanvasPoint,
    pub end: CanvasPoint,
}

impl CanvasRect {
    pub const fn new(start: CanvasPoint, end: CanvasPoint) -> Self {
        Self { start, end }
    }

    /// The whole canvas.
    pub const fn whole() -> Self {
        Self::new(CanvasPoint::new(0.0, 0.0), CanvasPoint::new(1.0, 1.0))
    }

    pub fn width(&self) -> f32 {
        (self.end.x - self.start.x).abs()
    }

    pub fn height(&self) -> f32 {
        (self.end.y - self.start.y).abs()
    }

    /// The aspect ratio of this slice of a canvas of the given pixel size.
    pub fn aspect_ratio(&self, canvas_width: u32, canvas_height: u32) -> Option<f32> {
        let width = self.width() * canvas_width as f32;
        let height = self.height() * canvas_height as f32;
        (height > 0.0 && width > 0.0).then_some(width / height)
    }
}

/// A quarter-turn applied to one screen.
///
/// Screens are hung sideways; canvases are not authored sideways. The turn belongs to the output
/// showing the slice, so turning one screen leaves the canvas and every other screen alone.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionRotation {
    #[default]
    None,
    Clockwise90,
    Half,
    CounterClockwise90,
}

impl RegionRotation {
    pub fn degrees(self) -> u16 {
        match self {
            Self::None => 0,
            Self::Clockwise90 => 90,
            Self::Half => 180,
            Self::CounterClockwise90 => 270,
        }
    }

    /// Whether this turn swaps the width and height of what it is applied to.
    pub fn swaps_axes(self) -> bool {
        matches!(self, Self::Clockwise90 | Self::CounterClockwise90)
    }

    /// The size the region occupies on screen once turned.
    pub fn applied_to(self, width: u32, height: u32) -> (u32, u32) {
        if self.swaps_axes() {
            (height, width)
        } else {
            (width, height)
        }
    }
}

/// How a region is fitted into the screen showing it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionFit {
    /// Fill the screen, cropping whatever overflows.
    #[default]
    Fill,
    /// Fit the whole region on screen, leaving bars where it does not reach.
    Contain,
    /// Stretch to the screen, changing the shape of the picture.
    Stretch,
}

/// One screen's view of the canvas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayRegion {
    pub id: String,
    pub name: String,
    /// The slice of the canvas this screen shows.
    pub source: CanvasRect,
    pub rotation: RegionRotation,
    pub fit: RegionFit,
    pub enabled: bool,
}

impl DisplayRegion {
    /// A region showing the whole canvas the right way up.
    pub fn whole(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source: CanvasRect::whole(),
            rotation: RegionRotation::None,
            fit: RegionFit::default(),
            enabled: true,
        }
    }

    /// The size this region wants on screen, in pixels of the given canvas.
    pub fn presented_size(&self, canvas_width: u32, canvas_height: u32) -> (u32, u32) {
        let width = (self.source.width() * canvas_width as f32).round().max(0.0) as u32;
        let height = (self.source.height() * canvas_height as f32)
            .round()
            .max(0.0) as u32;
        self.rotation.applied_to(width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_defaults_to_the_whole_canvas_the_right_way_up() {
        let region = DisplayRegion::whole("main", "Main");
        assert_eq!(region.source, CanvasRect::whole());
        assert_eq!(region.rotation, RegionRotation::None);
        assert_eq!(region.presented_size(1920, 1080), (1920, 1080));
    }

    #[test]
    fn a_quarter_turn_stands_a_slice_on_its_side() {
        let mut portrait = DisplayRegion::whole("wall", "Portrait wall");
        portrait.rotation = RegionRotation::Clockwise90;
        // A 16:9 slice hung sideways is presented 9:16.
        assert_eq!(portrait.presented_size(1920, 1080), (1080, 1920));
        assert_eq!(portrait.rotation.degrees(), 90);
    }

    #[test]
    fn a_half_turn_leaves_the_shape_alone() {
        let mut inverted = DisplayRegion::whole("ceiling", "Ceiling");
        inverted.rotation = RegionRotation::Half;
        assert_eq!(inverted.presented_size(1920, 1080), (1920, 1080));
        assert!(!inverted.rotation.swaps_axes());
    }

    #[test]
    fn the_reference_layout_slices_one_canvas_three_ways() {
        // A central 16:9 slice stood vertically, with a strip either side of it.
        let centre = DisplayRegion {
            source: CanvasRect::new(CanvasPoint::new(0.25, 0.0), CanvasPoint::new(0.75, 1.0)),
            rotation: RegionRotation::Clockwise90,
            ..DisplayRegion::whole("centre", "HDMI 1")
        };
        let left = DisplayRegion {
            source: CanvasRect::new(CanvasPoint::new(0.0, 0.0), CanvasPoint::new(0.25, 1.0)),
            ..DisplayRegion::whole("left", "HDMI 2")
        };
        let right = DisplayRegion {
            source: CanvasRect::new(CanvasPoint::new(0.75, 0.0), CanvasPoint::new(1.0, 1.0)),
            ..DisplayRegion::whole("right", "HDMI 3")
        };
        assert_eq!(centre.presented_size(1920, 1080), (1080, 960));
        assert_eq!(left.presented_size(1920, 1080), (480, 1080));
        assert_eq!(right.presented_size(1920, 1080), (480, 1080));
        // Turning the centre screen left the two beside it exactly as they were.
        assert_eq!(left.rotation, RegionRotation::None);
        assert_eq!(right.rotation, RegionRotation::None);
    }

    #[test]
    fn a_slice_reports_its_own_aspect_ratio() {
        let half = CanvasRect::new(CanvasPoint::new(0.0, 0.0), CanvasPoint::new(0.5, 1.0));
        assert_eq!(half.aspect_ratio(1920, 1080), Some(960.0 / 1080.0));
        let empty = CanvasRect::new(CanvasPoint::new(0.5, 0.0), CanvasPoint::new(0.5, 1.0));
        assert_eq!(empty.aspect_ratio(1920, 1080), None);
    }
}
