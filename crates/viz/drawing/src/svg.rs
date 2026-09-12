//! SVG, read as line work.
//!
//! Only the geometry matters here: shapes and paths become polylines, `transform` attributes are
//! composed down the element tree, and curves are flattened. Fills, strokes, styles, text, images
//! and clip paths are ignored — a placed drawing is a backdrop, drawn in the plan's own line
//! colour.
//!
//! SVG's Y axis points down and a plan's points up, so every Y is negated as it is read. The
//! result is Y-up millimetres, the same space the DXF reader produces.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{
    DrawingError, DrawingUnits, FLATTEN_TOLERANCE_MM, Polyline, UnderlayGeometry, arc_points,
    finish,
};

mod path;

use path::path_polylines;

/// A 2D affine transform, in the same `a b c d e f` order SVG's `matrix()` uses.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Transform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Transform {
    pub(crate) const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn then(self, inner: Self) -> Self {
        Self {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    fn apply(self, point: [f64; 2]) -> [f64; 2] {
        [
            self.a * point[0] + self.c * point[1] + self.e,
            self.b * point[0] + self.d * point[1] + self.f,
        ]
    }

    pub(crate) fn magnitude(self) -> f64 {
        self.a.hypot(self.b).max(self.c.hypot(self.d)).max(1e-9)
    }
}

/// Parse one `transform` attribute: the listed operations apply right to left.
fn parse_transform(value: &str) -> Transform {
    let mut result = Transform::IDENTITY;
    let mut rest = value;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim_matches(|c: char| !c.is_ascii_alphabetic());
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let numbers = numbers(&rest[open + 1..open + close]);
        let step = match (name, numbers.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => Transform {
                a: *a,
                b: *b,
                c: *c,
                d: *d,
                e: *e,
                f: *f,
            },
            ("translate", [x, rest @ ..]) => Transform {
                e: *x,
                f: rest.first().copied().unwrap_or(0.0),
                ..Transform::IDENTITY
            },
            ("scale", [x, rest @ ..]) => Transform {
                a: *x,
                d: rest.first().copied().unwrap_or(*x),
                ..Transform::IDENTITY
            },
            ("rotate", [degrees, rest @ ..]) => {
                let (sin, cos) = degrees.to_radians().sin_cos();
                let turn = Transform {
                    a: cos,
                    b: sin,
                    c: -sin,
                    d: cos,
                    e: 0.0,
                    f: 0.0,
                };
                match rest {
                    [x, y] => Transform {
                        e: *x,
                        f: *y,
                        ..Transform::IDENTITY
                    }
                    .then(turn)
                    .then(Transform {
                        e: -*x,
                        f: -*y,
                        ..Transform::IDENTITY
                    }),
                    _ => turn,
                }
            }
            ("skewX", [degrees]) => Transform {
                c: degrees.to_radians().tan(),
                ..Transform::IDENTITY
            },
            ("skewY", [degrees]) => Transform {
                b: degrees.to_radians().tan(),
                ..Transform::IDENTITY
            },
            _ => Transform::IDENTITY,
        };
        result = result.then(step);
        rest = &rest[open + close + 1..];
    }
    result
}

pub(crate) fn numbers(value: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        if bytes[index] == b'-' || bytes[index] == b'+' {
            index += 1;
        }
        while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
            index += 1;
        }
        if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
            index += 1;
            if index < bytes.len() && (bytes[index] == b'-' || bytes[index] == b'+') {
                index += 1;
            }
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
        }
        if index > start {
            if let Ok(number) = value[start..index].parse() {
                numbers.push(number);
            }
        } else {
            index += 1;
        }
    }
    numbers
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == name.as_bytes())
            .then(|| String::from_utf8_lossy(&attribute.value).into_owned())
    })
}

fn number_attribute(element: &BytesStart<'_>, name: &str) -> f64 {
    attribute(element, name)
        .and_then(|value| numbers(&value).first().copied())
        .unwrap_or_default()
}

/// A length with a CSS unit, in millimetres. Bare numbers are user units, which have no size yet.
fn length_millimetres(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let number = numbers(trimmed).first().copied()?;
    let unit: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphabetic() || *c == '%')
        .collect();
    match unit.to_ascii_lowercase().as_str() {
        "mm" => Some(number),
        "cm" => Some(number * 10.0),
        "m" => Some(number * 1000.0),
        "in" => Some(number * 25.4),
        "pt" => Some(number * 25.4 / 72.0),
        "pc" => Some(number * 25.4 / 6.0),
        // CSS pixels are 1/96 inch, which is what a drawing exported at "96 dpi" means.
        "px" | "" => None,
        _ => None,
    }
}

/// How the root element maps its user units onto millimetres: a scale, the view box's own origin,
/// and what that says about the drawing's units.
fn root_scale(element: &BytesStart<'_>) -> (f64, [f64; 2], DrawingUnits) {
    let view_box = attribute(element, "viewBox")
        .map(|value| numbers(&value))
        .filter(|values| values.len() == 4);
    let width = attribute(element, "width").and_then(|value| length_millimetres(&value));
    let origin = view_box
        .as_ref()
        .map(|values| [values[0], values[1]])
        .unwrap_or([0.0, 0.0]);
    match (view_box, width) {
        (Some(view_box), Some(width)) if view_box[2] > 0.0 => {
            (width / view_box[2], origin, DrawingUnits::Millimetres)
        }
        _ => (1.0, origin, DrawingUnits::Assumed),
    }
}

fn shape_polylines(
    element: &BytesStart<'_>,
    name: &str,
    tolerance: f64,
) -> Vec<(Vec<[f64; 2]>, bool)> {
    let points = |value: Option<String>| {
        value
            .map(|value| {
                numbers(&value)
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| [pair[0], pair[1]])
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    match name {
        "line" => vec![(
            vec![
                [
                    number_attribute(element, "x1"),
                    number_attribute(element, "y1"),
                ],
                [
                    number_attribute(element, "x2"),
                    number_attribute(element, "y2"),
                ],
            ],
            false,
        )],
        "polyline" => vec![(points(attribute(element, "points")), false)],
        "polygon" => vec![(points(attribute(element, "points")), true)],
        "rect" => {
            let x = number_attribute(element, "x");
            let y = number_attribute(element, "y");
            let width = number_attribute(element, "width");
            let height = number_attribute(element, "height");
            vec![(
                vec![
                    [x, y],
                    [x + width, y],
                    [x + width, y + height],
                    [x, y + height],
                ],
                true,
            )]
        }
        "circle" => vec![(
            arc_points(
                [
                    number_attribute(element, "cx"),
                    number_attribute(element, "cy"),
                ],
                number_attribute(element, "r"),
                0.0,
                std::f64::consts::TAU,
                tolerance,
            ),
            true,
        )],
        "ellipse" => {
            let centre = [
                number_attribute(element, "cx"),
                number_attribute(element, "cy"),
            ];
            let rx = number_attribute(element, "rx");
            let ry = number_attribute(element, "ry");
            vec![(
                arc_points(
                    [0.0, 0.0],
                    rx.max(ry),
                    0.0,
                    std::f64::consts::TAU,
                    tolerance,
                )
                .into_iter()
                .map(|point| {
                    let radius = rx.max(ry).max(1e-9);
                    [
                        centre[0] + point[0] * rx / radius,
                        centre[1] + point[1] * ry / radius,
                    ]
                })
                .collect(),
                true,
            )]
        }
        "path" => path_polylines(&attribute(element, "d").unwrap_or_default(), tolerance),
        _ => Vec::new(),
    }
}

/// Draw one element, and return the transform its children inherit.
fn visit(
    element: &BytesStart<'_>,
    tag: &str,
    parent: Transform,
    root: &mut Option<DrawingUnits>,
    polylines: &mut Vec<Polyline>,
) -> Transform {
    let own = attribute(element, "transform")
        .map(|value| parse_transform(&value))
        .unwrap_or(Transform::IDENTITY);
    let transform = if tag == "svg" && root.is_none() {
        let (scale, origin, units) = root_scale(element);
        *root = Some(units);
        // One transform carries the drawing's physical scale, its view box's origin, and the flip
        // from SVG's Y-down space into the plan's Y-up millimetres; everything below inherits it.
        Transform {
            a: scale,
            b: 0.0,
            c: 0.0,
            d: -scale,
            e: -origin[0] * scale,
            f: origin[1] * scale,
        }
        .then(own)
    } else {
        parent.then(own)
    };
    let tolerance = FLATTEN_TOLERANCE_MM / transform.magnitude();
    let layer = attribute(element, "id").unwrap_or_default();
    for (points, closed) in shape_polylines(element, tag, tolerance) {
        if points.len() < 2 {
            continue;
        }
        polylines.push(Polyline::new(
            points
                .into_iter()
                .map(|point| transform.apply(point))
                .collect(),
            closed,
            &layer,
        ));
    }
    transform
}

/// Read an SVG drawing into millimetre polylines, Y pointing up.
pub fn parse_svg(name: &str, bytes: &[u8]) -> Result<UnderlayGeometry, DrawingError> {
    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<Transform> = vec![Transform::IDENTITY];
    let mut polylines: Vec<Polyline> = Vec::new();
    let mut root: Option<DrawingUnits> = None;
    let mut buffer = Vec::new();

    loop {
        let event =
            reader
                .read_event_into(&mut buffer)
                .map_err(|error| DrawingError::Malformed {
                    name: name.to_owned(),
                    detail: error.to_string(),
                })?;
        match event {
            Event::Eof => break,
            Event::Start(element) => {
                let tag = String::from_utf8_lossy(element.local_name().as_ref()).into_owned();
                let parent = *stack.last().unwrap_or(&Transform::IDENTITY);
                stack.push(visit(&element, &tag, parent, &mut root, &mut polylines));
            }
            Event::Empty(element) => {
                let tag = String::from_utf8_lossy(element.local_name().as_ref()).into_owned();
                let parent = *stack.last().unwrap_or(&Transform::IDENTITY);
                visit(&element, &tag, parent, &mut root, &mut polylines);
            }
            Event::End(_) if stack.len() > 1 => {
                stack.pop();
            }
            _ => {}
        }
        buffer.clear();
    }
    let Some(units) = root else {
        return Err(DrawingError::Malformed {
            name: name.to_owned(),
            detail: "no <svg> element was found".to_owned(),
        });
    };
    // The root transform already produced millimetres, so `finish` has nothing left to convert.
    finish(name, polylines, units)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(attributes: &str, body: &str) -> String {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" {attributes}>{body}</svg>")
    }

    #[test]
    fn a_physical_width_scales_user_units_to_millimetres() {
        let source = svg(
            "width=\"100mm\" height=\"50mm\" viewBox=\"0 0 1000 500\"",
            "<line x1=\"0\" y1=\"0\" x2=\"1000\" y2=\"0\"/>",
        );
        let geometry = parse_svg("plan.svg", source.as_bytes()).expect("geometry");
        assert_eq!(geometry.units, DrawingUnits::Millimetres);
        assert!((geometry.polylines[0].points[1][0] - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_drawing_without_a_physical_width_is_read_as_millimetres_and_says_so() {
        let source = svg(
            "viewBox=\"0 0 100 100\"",
            "<line x1=\"0\" y1=\"0\" x2=\"100\" y2=\"0\"/>",
        );
        let geometry = parse_svg("plan.svg", source.as_bytes()).expect("geometry");
        assert_eq!(geometry.units, DrawingUnits::Assumed);
        assert!(geometry.units.label().contains("names no unit"));
        assert!((geometry.polylines[0].points[1][0] - 100.0).abs() < 1e-9);
    }

    #[test]
    fn y_is_flipped_so_the_drawing_arrives_y_up() {
        let source = svg(
            "viewBox=\"0 0 100 100\"",
            "<line x1=\"0\" y1=\"10\" x2=\"0\" y2=\"90\"/>",
        );
        let geometry = parse_svg("plan.svg", source.as_bytes()).expect("geometry");
        let points = &geometry.polylines[0].points;
        assert_eq!(points[0], [0.0, -10.0]);
        assert_eq!(points[1], [0.0, -90.0]);
    }

    #[test]
    fn a_view_box_origin_moves_the_drawing_onto_it() {
        let source = svg(
            "viewBox=\"50 0 100 100\"",
            "<line x1=\"50\" y1=\"0\" x2=\"150\" y2=\"0\"/>",
        );
        let geometry = parse_svg("plan.svg", source.as_bytes()).expect("geometry");
        assert_eq!(geometry.polylines[0].points[0], [0.0, 0.0]);
        assert_eq!(geometry.polylines[0].points[1], [100.0, 0.0]);
    }

    #[test]
    fn group_transforms_compose_down_the_tree() {
        let source = svg(
            "viewBox=\"0 0 100 100\"",
            "<g transform=\"translate(10 0)\"><g transform=\"scale(2)\">             <line x1=\"0\" y1=\"0\" x2=\"5\" y2=\"0\"/></g></g>",
        );
        let geometry = parse_svg("plan.svg", source.as_bytes()).expect("geometry");
        let points = &geometry.polylines[0].points;
        assert_eq!(points[0], [10.0, 0.0]);
        assert_eq!(points[1], [20.0, 0.0]);
    }

    #[test]
    fn an_empty_element_does_not_inherit_its_own_transform_to_its_siblings() {
        let source = svg(
            "viewBox=\"0 0 100 100\"",
            "<line transform=\"translate(50 0)\" x1=\"0\" y1=\"0\" x2=\"10\" y2=\"0\"/>             <line x1=\"0\" y1=\"0\" x2=\"10\" y2=\"0\"/>",
        );
        let geometry = parse_svg("plan.svg", source.as_bytes()).expect("geometry");
        assert_eq!(geometry.polylines[0].points[0], [50.0, 0.0]);
        assert_eq!(geometry.polylines[1].points[0], [0.0, 0.0]);
    }

    #[test]
    fn rectangles_and_circles_become_closed_runs() {
        let source = svg(
            "viewBox=\"0 0 100 100\"",
            "<rect x=\"0\" y=\"0\" width=\"10\" height=\"20\"/><circle cx=\"0\" cy=\"0\" r=\"30\"/>",
        );
        let geometry = parse_svg("plan.svg", source.as_bytes()).expect("geometry");
        assert_eq!(geometry.polylines[0].points.len(), 4);
        assert!(geometry.polylines[0].closed);
        assert!(geometry.polylines[1].points.len() > 8);
    }

    #[test]
    fn a_file_without_an_svg_root_is_refused_by_name() {
        let error = parse_svg("plan.svg", b"<html><body/></html>").expect_err("refused");
        assert!(error.to_string().contains("plan.svg"));
    }
}
