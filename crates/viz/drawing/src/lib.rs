//! Venue drawings placed under a CAD plan.
//!
//! A ground plan, a section or a truss layout arrives as DXF or SVG and is reduced here to what
//! both consumers can draw: polylines in millimetres. The Architect's viewport paints them with
//! the same line renderer it paints the rig with, and the PDF writer emits them as page paths, so
//! neither side has to understand a drawing format.
//!
//! Curves are flattened at import. A drawing is a backdrop an operator places, scales and prints —
//! not a document this application edits — so nothing here preserves the source's own structure
//! beyond the layer each polyline came from.

mod dxf;
mod svg;

pub use dxf::parse_dxf;
pub use svg::parse_svg;

use serde::{Deserialize, Serialize};

/// How far a flattened curve may depart from the true arc, in millimetres of the source drawing.
pub const FLATTEN_TOLERANCE_MM: f64 = 0.5;

/// The largest source file an import accepts, matching the media-fallback limit.
pub const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;

/// The most points one drawing may carry, past which a plan is too heavy to place and print.
pub const MAX_POINTS: usize = 400_000;

/// One open or closed run of straight segments, in millimetres, in the drawing's own space.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Polyline {
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
    /// The source layer this run came from, kept so a later version can switch layers off.
    pub layer: String,
}

impl Polyline {
    fn new(points: Vec<[f64; 2]>, closed: bool, layer: &str) -> Self {
        Self {
            points,
            closed,
            layer: layer.to_owned(),
        }
    }
}

/// A parsed drawing: its polylines and the box they occupy, both in millimetres.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnderlayGeometry {
    pub polylines: Vec<Polyline>,
    /// `[minX, minY, maxX, maxY]`; all zero when the drawing is empty.
    pub extents_millimetres: [f64; 4],
    /// What the source said its units were, so the import can say what it assumed.
    pub units: DrawingUnits,
}

/// The unit the source drawing was read in, before conversion to millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DrawingUnits {
    Millimetres,
    Centimetres,
    Metres,
    Inches,
    Feet,
    /// The file named no unit; its numbers were read as millimetres.
    #[default]
    Assumed,
}

impl DrawingUnits {
    /// Millimetres per unit of the source drawing.
    fn scale(self) -> f64 {
        match self {
            Self::Millimetres | Self::Assumed => 1.0,
            Self::Centimetres => 10.0,
            Self::Metres => 1000.0,
            Self::Inches => 25.4,
            Self::Feet => 304.8,
        }
    }

    /// The operator-facing name, for the sentence an import reports.
    pub fn label(self) -> &'static str {
        match self {
            Self::Millimetres => "millimetres",
            Self::Centimetres => "centimetres",
            Self::Metres => "metres",
            Self::Inches => "inches",
            Self::Feet => "feet",
            Self::Assumed => "millimetres (the file names no unit)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DrawingError {
    #[error("{name} is {bytes} bytes; a drawing may be at most {max} bytes")]
    TooLarge {
        name: String,
        bytes: usize,
        max: usize,
    },
    #[error(
        "{name} holds more than {max} points. Simplify the drawing in your CAD application, or \
         export only the layers the plan needs."
    )]
    TooManyPoints { name: String, max: usize },
    #[error("{name} holds no lines this version can draw: {detail}")]
    Empty { name: String, detail: String },
    #[error("{name} is not a drawing this version can read: {detail}")]
    Malformed { name: String, detail: String },
}

/// Read a drawing from its bytes, choosing the parser by file extension.
pub fn parse_drawing(name: &str, bytes: &[u8]) -> Result<UnderlayGeometry, DrawingError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(DrawingError::TooLarge {
            name: name.to_owned(),
            bytes: bytes.len(),
            max: MAX_SOURCE_BYTES,
        });
    }
    let lowercase = name.to_ascii_lowercase();
    if lowercase.ends_with(".dxf") {
        parse_dxf(name, bytes)
    } else if lowercase.ends_with(".svg") {
        parse_svg(name, bytes)
    } else {
        Err(DrawingError::Malformed {
            name: name.to_owned(),
            detail: "only DXF and SVG drawings can be placed".to_owned(),
        })
    }
}

/// Scale every point to millimetres, drop runs too short to draw, and measure the result.
pub(crate) fn finish(
    name: &str,
    mut polylines: Vec<Polyline>,
    units: DrawingUnits,
) -> Result<UnderlayGeometry, DrawingError> {
    let scale = units.scale();
    let mut points = 0;
    for polyline in &mut polylines {
        for point in &mut polyline.points {
            point[0] *= scale;
            point[1] *= scale;
        }
        points += polyline.points.len();
    }
    if points > MAX_POINTS {
        return Err(DrawingError::TooManyPoints {
            name: name.to_owned(),
            max: MAX_POINTS,
        });
    }
    polylines.retain(|polyline| polyline.points.len() >= 2);
    if polylines.is_empty() {
        return Err(DrawingError::Empty {
            name: name.to_owned(),
            detail: "no lines, polylines, arcs or circles were found".to_owned(),
        });
    }
    let mut extents = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for polyline in &polylines {
        for point in &polyline.points {
            extents[0] = extents[0].min(point[0]);
            extents[1] = extents[1].min(point[1]);
            extents[2] = extents[2].max(point[0]);
            extents[3] = extents[3].max(point[1]);
        }
    }
    Ok(UnderlayGeometry {
        polylines,
        extents_millimetres: extents,
        units,
    })
}

/// Flatten a circular arc into segments no further than the tolerance from the true curve.
///
/// `start` and `sweep` are in radians; a full circle is a sweep of a full turn. The segment count
/// comes from the sagitta of one segment, so a large radius is subdivided more finely than a small
/// one and both stay within the same drawn error.
pub(crate) fn arc_points(
    centre: [f64; 2],
    radius: f64,
    start: f64,
    sweep: f64,
    tolerance: f64,
) -> Vec<[f64; 2]> {
    if radius <= 0.0 || sweep == 0.0 {
        return Vec::new();
    }
    let usable = tolerance.min(radius).max(f64::EPSILON);
    let step = 2.0 * (1.0 - usable / radius).clamp(-1.0, 1.0).acos();
    let segments = if step > 0.0 {
        ((sweep.abs() / step).ceil() as usize).clamp(2, 4096)
    } else {
        16
    };
    (0..=segments)
        .map(|index| {
            let angle = start + sweep * (index as f64 / segments as f64);
            [
                centre[0] + radius * angle.cos(),
                centre[1] + radius * angle.sin(),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flattened_arc_stays_within_the_tolerance() {
        let points = arc_points([0.0, 0.0], 1000.0, 0.0, std::f64::consts::TAU, 0.5);
        assert!(points.len() > 8);
        for window in points.windows(2) {
            let midpoint = [
                (window[0][0] + window[1][0]) / 2.0,
                (window[0][1] + window[1][1]) / 2.0,
            ];
            let radius = midpoint[0].hypot(midpoint[1]);
            assert!(
                (1000.0 - radius) <= 0.5 + 1e-6,
                "chord midpoint is {radius} from the centre"
            );
        }
    }

    #[test]
    fn units_convert_to_millimetres() {
        let geometry = finish(
            "plan.dxf",
            vec![Polyline::new(vec![[0.0, 0.0], [12.0, 0.0]], false, "0")],
            DrawingUnits::Inches,
        )
        .expect("geometry");
        assert_eq!(geometry.polylines[0].points[1], [304.79999999999995, 0.0]);
        assert_eq!(geometry.extents_millimetres[2], 304.79999999999995);
    }

    #[test]
    fn a_drawing_without_drawable_lines_is_refused_by_name() {
        let error = finish("empty.dxf", Vec::new(), DrawingUnits::Millimetres)
            .expect_err("an empty drawing is refused");
        assert!(matches!(error, DrawingError::Empty { ref name, .. } if name == "empty.dxf"));
        assert!(error.to_string().contains("empty.dxf"));
    }

    #[test]
    fn an_oversized_file_is_refused_by_name() {
        let error = parse_drawing("huge.dxf", &vec![0u8; MAX_SOURCE_BYTES + 1])
            .expect_err("an oversized drawing is refused");
        assert!(error.to_string().contains("huge.dxf"));
    }

    #[test]
    fn an_unknown_extension_is_refused() {
        let error = parse_drawing("plan.pdf", b"%PDF-").expect_err("only DXF and SVG");
        assert!(error.to_string().contains("DXF and SVG"));
    }
}
