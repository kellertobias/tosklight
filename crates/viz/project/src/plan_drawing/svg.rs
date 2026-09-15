//! The shipped model drawings reduced to what a plan draws: filled silhouette loops and visible
//! polylines, in the drawing's own page millimetres with y down.
//!
//! This file is compiled twice. `build.rs` includes it to read every drawing in
//! `assets/models/2d` and write each one as a compact binary blob, and the crate includes it to
//! read those blobs back — so the reader and the writer can never disagree about the format. It
//! therefore uses nothing but `std`.
//!
//! The drawings are meant to be edited by hand, so the reader takes what a vector editor writes:
//! curves and arcs are followed as short straight runs, `rect`, `circle`, `ellipse` and `line`
//! shapes are read as outlines, and `transform` attributes on groups and shapes are applied.

#![allow(dead_code)]

/// One point in page millimetres, y down.
pub type Point = (f32, f32);

/// A drawing reduced to its geometry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing {
    /// Closed loops of the covered area, filled even-odd: a loop inside another is a hole.
    pub silhouette: Vec<Vec<Point>>,
    /// Visible edges. A closed shape repeats its first point at the end.
    pub lines: Vec<Vec<Point>>,
}

/// An affine transform `[a, b, c, d, e, f]`, mapping `(x, y)` to `(a x + c y + e, b x + d y + f)`.
type Matrix = [f32; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// How many straight runs follow one curve.
const CURVE_STEPS: usize = 12;

/// Read a drawing. Everything under `silhouette` is area, everything under `lines` is linework
/// (`base`, `yoke`, `head`, `hardware` or any other group), and `origin` is ignored.
pub fn parse(svg: &str) -> Drawing {
    let mut drawing = Drawing::default();
    let mut groups: Vec<(String, Matrix)> = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if let Some(comment) = rest.strip_prefix("!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some(end) = rest.find('>') else {
            break;
        };
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if let Some(closing) = tag.strip_prefix('/') {
            if closing.trim() == "g" {
                groups.pop();
            }
            continue;
        }
        let name_end = tag
            .find(|character: char| character.is_ascii_whitespace() || character == '/')
            .unwrap_or(tag.len());
        let name = &tag[..name_end];
        let self_closing = tag.trim_end().ends_with('/');
        let parent = groups.last().map_or(IDENTITY, |(_, matrix)| *matrix);
        let matrix = attribute(tag, "transform").map_or(parent, |transform| {
            multiply(parent, parse_transform(transform))
        });
        if name == "g" {
            if !self_closing {
                groups.push((attribute(tag, "id").unwrap_or_default().to_owned(), matrix));
            }
            continue;
        }
        if groups.iter().any(|(group, _)| group == "origin") {
            continue;
        }
        let own_id = attribute(tag, "id").unwrap_or_default();
        let subpaths = shape(name, tag);
        if subpaths.is_empty() {
            continue;
        }
        let silhouette = own_id.starts_with("silhouette")
            || groups
                .iter()
                .any(|(group, _)| group.starts_with("silhouette"));
        let linework = groups.iter().any(|(group, _)| group == "lines");
        for points in subpaths.into_iter().filter(|points| points.len() >= 2) {
            let points: Vec<Point> = points
                .into_iter()
                .map(|point| apply(matrix, point))
                .collect();
            if silhouette {
                if points.len() >= 3 {
                    drawing.silhouette.push(points);
                }
            } else if linework {
                drawing.lines.push(points);
            }
        }
    }
    drawing
}

/// The outline of one SVG shape element, in its own coordinates.
fn shape(name: &str, tag: &str) -> Vec<Vec<Point>> {
    let number = |key: &str| attribute(tag, key).and_then(|value| numbers(value).first().copied());
    match name {
        "path" => attribute(tag, "d").map(path_data).unwrap_or_default(),
        "polyline" | "polygon" => attribute(tag, "points")
            .map(|points| {
                let mut points = pairs(&numbers(points));
                if name == "polygon"
                    && let Some(first) = points.first().copied()
                {
                    points.push(first);
                }
                vec![points]
            })
            .unwrap_or_default(),
        "line" => vec![vec![
            (number("x1").unwrap_or(0.0), number("y1").unwrap_or(0.0)),
            (number("x2").unwrap_or(0.0), number("y2").unwrap_or(0.0)),
        ]],
        "rect" => {
            let (x, y) = (number("x").unwrap_or(0.0), number("y").unwrap_or(0.0));
            let (Some(width), Some(height)) = (number("width"), number("height")) else {
                return Vec::new();
            };
            vec![vec![
                (x, y),
                (x + width, y),
                (x + width, y + height),
                (x, y + height),
                (x, y),
            ]]
        }
        "circle" | "ellipse" => {
            let (cx, cy) = (number("cx").unwrap_or(0.0), number("cy").unwrap_or(0.0));
            let (rx, ry) = if name == "circle" {
                let radius = number("r").unwrap_or(0.0);
                (radius, radius)
            } else {
                (number("rx").unwrap_or(0.0), number("ry").unwrap_or(0.0))
            };
            if rx <= 0.0 || ry <= 0.0 {
                return Vec::new();
            }
            let steps = CURVE_STEPS * 3;
            vec![
                (0..=steps)
                    .map(|step| {
                        let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
                        (cx + rx * angle.cos(), cy + ry * angle.sin())
                    })
                    .collect(),
            ]
        }
        _ => Vec::new(),
    }
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut search = tag;
    loop {
        let at = search.find(name)?;
        let before = search[..at].chars().next_back();
        let after = &search[at + name.len()..];
        let after = after.trim_start();
        if before.is_some_and(|character| character.is_ascii_whitespace())
            && let Some(value) = after.strip_prefix('=')
        {
            let value = value.trim_start();
            let quote = value.chars().next()?;
            if quote == '"' || quote == '\'' {
                let value = &value[1..];
                return Some(&value[..value.find(quote)?]);
            }
        }
        search = &search[at + name.len()..];
    }
}

fn numbers(text: &str) -> Vec<f32> {
    tokens(text)
        .into_iter()
        .filter_map(|token| match token {
            Token::Number(value) => Some(value),
            Token::Command(_) => None,
        })
        .collect()
}

fn pairs(values: &[f32]) -> Vec<Point> {
    values
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[x, y]| (*x, *y))
        .collect()
}

/// `translate`, `scale`, `rotate`, `skewX`, `skewY` and `matrix`, composed left to right.
fn parse_transform(text: &str) -> Matrix {
    let mut matrix = IDENTITY;
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().trim_start_matches(',').trim();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let values = numbers(&rest[open + 1..open + close]);
        let value = |index: usize, fallback: f32| values.get(index).copied().unwrap_or(fallback);
        let next = match name {
            "translate" => [1.0, 0.0, 0.0, 1.0, value(0, 0.0), value(1, 0.0)],
            "scale" => {
                let x = value(0, 1.0);
                [x, 0.0, 0.0, value(1, x), 0.0, 0.0]
            }
            "rotate" => {
                let (sin, cos) = value(0, 0.0).to_radians().sin_cos();
                let (cx, cy) = (value(1, 0.0), value(2, 0.0));
                let turn = [cos, sin, -sin, cos, 0.0, 0.0];
                multiply(
                    [1.0, 0.0, 0.0, 1.0, cx, cy],
                    multiply(turn, [1.0, 0.0, 0.0, 1.0, -cx, -cy]),
                )
            }
            "skewX" => [1.0, 0.0, value(0, 0.0).to_radians().tan(), 1.0, 0.0, 0.0],
            "skewY" => [1.0, value(0, 0.0).to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
            "matrix" if values.len() >= 6 => [
                values[0], values[1], values[2], values[3], values[4], values[5],
            ],
            _ => IDENTITY,
        };
        matrix = multiply(matrix, next);
        rest = &rest[open + close + 1..];
    }
    matrix
}

/// `outer` after `inner`: a point is transformed by `inner` first.
fn multiply(outer: Matrix, inner: Matrix) -> Matrix {
    let [a, b, c, d, e, f] = outer;
    let [g, h, i, j, k, l] = inner;
    [
        a * g + c * h,
        b * g + d * h,
        a * i + c * j,
        b * i + d * j,
        a * k + c * l + e,
        b * k + d * l + f,
    ]
}

fn apply(matrix: Matrix, (x, y): Point) -> Point {
    let [a, b, c, d, e, f] = matrix;
    (a * x + c * y + e, b * x + d * y + f)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Token {
    Command(char),
    Number(f32),
}

fn tokens(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_alphabetic() && !matches!(byte, b'e' | b'E') {
            out.push(Token::Command(byte as char));
            index += 1;
            continue;
        }
        if !(byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.')) {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        let mut seen_dot = byte == b'.';
        while index < bytes.len() {
            let next = bytes[index];
            if next.is_ascii_digit() {
                index += 1;
            } else if next == b'.' && !seen_dot {
                seen_dot = true;
                index += 1;
            } else if matches!(next, b'e' | b'E') {
                index += 1;
                if index < bytes.len() && matches!(bytes[index], b'-' | b'+') {
                    index += 1;
                }
            } else {
                break;
            }
        }
        if let Ok(value) = text[start..index].parse::<f32>() {
            out.push(Token::Number(value));
        }
    }
    out
}

/// Every path command in either case. Curves and arcs are followed as short straight runs.
fn path_data(data: &str) -> Vec<Vec<Point>> {
    let tokens = tokens(data);
    let mut subpaths: Vec<Vec<Point>> = Vec::new();
    let mut current: Option<Vec<Point>> = None;
    let mut command = 'M';
    let mut at: Point = (0.0, 0.0);
    let mut start: Point = (0.0, 0.0);
    // The last control point, for the smooth curves that reflect it.
    let mut control: Option<(char, Point)> = None;
    let mut index = 0;
    while index < tokens.len() {
        if let Token::Command(next) = tokens[index] {
            command = next;
            index += 1;
            if command.eq_ignore_ascii_case(&'z') {
                if let Some(mut points) = current.take() {
                    if points.len() > 2 {
                        points.push(start);
                    }
                    subpaths.push(points);
                }
                at = start;
                control = None;
            }
            continue;
        }
        let lower = command.to_ascii_lowercase();
        let arity = match lower {
            'h' | 'v' => 1,
            's' | 'q' => 4,
            'c' => 6,
            'a' => 7,
            'z' => {
                index += 1;
                continue;
            }
            _ => 2,
        };
        let args = tokens[index..]
            .iter()
            .take(arity)
            .map_while(|token| match token {
                Token::Number(value) => Some(*value),
                Token::Command(_) => None,
            })
            .collect::<Vec<_>>();
        if args.len() < arity {
            index += args.len().max(1);
            continue;
        }
        index += arity;
        let relative = command.is_ascii_lowercase();
        let point = |x: f32, y: f32| {
            if relative {
                (at.0 + x, at.1 + y)
            } else {
                (x, y)
            }
        };
        let points = current.get_or_insert_with(Vec::new);
        let mut next_control = None;
        let end = match lower {
            'm' => {
                let end = point(args[0], args[1]);
                if let Some(points) = current.take()
                    && !points.is_empty()
                {
                    subpaths.push(points);
                }
                start = end;
                current = Some(vec![end]);
                // Coordinates after a move continue as lines.
                command = if relative { 'l' } else { 'L' };
                at = end;
                control = None;
                continue;
            }
            'h' => (if relative { at.0 + args[0] } else { args[0] }, at.1),
            'v' => (at.0, if relative { at.1 + args[0] } else { args[0] }),
            'c' | 's' => {
                let (first, second, end) = if lower == 'c' {
                    (
                        point(args[0], args[1]),
                        point(args[2], args[3]),
                        point(args[4], args[5]),
                    )
                } else {
                    let first = match control {
                        Some(('c', last)) => (2.0 * at.0 - last.0, 2.0 * at.1 - last.1),
                        _ => at,
                    };
                    (first, point(args[0], args[1]), point(args[2], args[3]))
                };
                points.extend(cubic(at, first, second, end));
                next_control = Some(('c', second));
                end
            }
            'q' | 't' => {
                let (handle, end) = if lower == 'q' {
                    (point(args[0], args[1]), point(args[2], args[3]))
                } else {
                    let handle = match control {
                        Some(('q', last)) => (2.0 * at.0 - last.0, 2.0 * at.1 - last.1),
                        _ => at,
                    };
                    (handle, point(args[0], args[1]))
                };
                points.extend(quadratic(at, handle, end));
                next_control = Some(('q', handle));
                end
            }
            'a' => {
                let end = point(args[5], args[6]);
                points.extend(arc(
                    at,
                    end,
                    args[0],
                    args[1],
                    args[2],
                    args[3] != 0.0,
                    args[4] != 0.0,
                ));
                end
            }
            _ => point(args[0], args[1]),
        };
        if !matches!(lower, 'c' | 's' | 'q' | 't' | 'a') {
            points.push(end);
        }
        at = end;
        control = next_control;
    }
    if let Some(points) = current
        && !points.is_empty()
    {
        subpaths.push(points);
    }
    subpaths
}

/// The points along a cubic Bézier from `from` to `end`, excluding `from`.
fn cubic(from: Point, first: Point, second: Point, end: Point) -> Vec<Point> {
    (1..=CURVE_STEPS)
        .map(|step| {
            let t = step as f32 / CURVE_STEPS as f32;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            (
                w[0] * from.0 + w[1] * first.0 + w[2] * second.0 + w[3] * end.0,
                w[0] * from.1 + w[1] * first.1 + w[2] * second.1 + w[3] * end.1,
            )
        })
        .collect()
}

/// The points along a quadratic Bézier from `from` to `end`, excluding `from`.
fn quadratic(from: Point, handle: Point, end: Point) -> Vec<Point> {
    (1..=CURVE_STEPS)
        .map(|step| {
            let t = step as f32 / CURVE_STEPS as f32;
            let u = 1.0 - t;
            (
                u * u * from.0 + 2.0 * u * t * handle.0 + t * t * end.0,
                u * u * from.1 + 2.0 * u * t * handle.1 + t * t * end.1,
            )
        })
        .collect()
}

/// The points along an elliptical arc from `from` to `to`, excluding `from` (SVG endpoint
/// parameterisation, converted to centre form).
fn arc(
    from: Point,
    to: Point,
    rx: f32,
    ry: f32,
    rotation: f32,
    large: bool,
    sweep: bool,
) -> Vec<Point> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-6 || ry < 1e-6 || (from.0 - to.0).abs() + (from.1 - to.1).abs() < 1e-6 {
        return vec![to];
    }
    let (sin, cos) = rotation.to_radians().sin_cos();
    let dx = (from.0 - to.0) / 2.0;
    let dy = (from.1 - to.1) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let scale = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if scale > 1.0 {
        rx *= scale.sqrt();
        ry *= scale.sqrt();
    }
    let numerator = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut factor = (numerator / denominator.max(1e-12)).sqrt();
    if large == sweep {
        factor = -factor;
    }
    let cx1 = factor * rx * y1 / ry;
    let cy1 = -factor * ry * x1 / rx;
    let centre = (
        cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0,
        sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0,
    );
    let angle = |ux: f32, uy: f32| uy.atan2(ux);
    let start = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry) - start;
    let tau = std::f32::consts::TAU;
    if sweep && delta < 0.0 {
        delta += tau;
    } else if !sweep && delta > 0.0 {
        delta -= tau;
    }
    let steps = ((delta.abs() / tau) * CURVE_STEPS as f32 * 4.0)
        .ceil()
        .max(1.0) as usize;
    (1..=steps)
        .map(|step| {
            if step == steps {
                return to;
            }
            let theta = start + delta * step as f32 / steps as f32;
            let (x, y) = (rx * theta.cos(), ry * theta.sin());
            (cos * x - sin * y + centre.0, sin * x + cos * y + centre.1)
        })
        .collect()
}

/// Write a drawing as: unit (f32), then the silhouette and the lines, each a loop count (u32)
/// followed by, per loop, a point count (u32) and its points as `i16` multiples of the unit. All
/// little-endian. The unit is a tenth of a millimetre unless the drawing is too large for that.
pub fn encode(drawing: &Drawing) -> Vec<u8> {
    let largest = drawing
        .silhouette
        .iter()
        .chain(&drawing.lines)
        .flatten()
        .fold(0.0_f32, |largest, (x, y)| largest.max(x.abs()).max(y.abs()));
    let unit = (largest / f32::from(i16::MAX)).max(0.1);
    let mut out = unit.to_le_bytes().to_vec();
    for loops in [&drawing.silhouette, &drawing.lines] {
        out.extend_from_slice(&(loops.len() as u32).to_le_bytes());
        for points in loops {
            out.extend_from_slice(&(points.len() as u32).to_le_bytes());
            for (x, y) in points {
                for value in [x, y] {
                    out.extend_from_slice(&((value / unit).round() as i16).to_le_bytes());
                }
            }
        }
    }
    out
}

/// Read what [`encode`] wrote, or `None` when the bytes are cut short.
pub fn decode(bytes: &[u8]) -> Option<Drawing> {
    let mut cursor = 0_usize;
    let mut take = |count: usize| -> Option<&[u8]> {
        let slice = bytes.get(cursor..cursor + count)?;
        cursor += count;
        Some(slice)
    };
    let unit = f32::from_le_bytes(take(4)?.try_into().ok()?);
    let mut drawing = Drawing::default();
    for target in 0..2 {
        let loops = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
        for _ in 0..loops {
            let count = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
            let raw = take(count.checked_mul(4)?)?;
            let points = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|[x0, x1, y0, y1]| {
                    let x = i16::from_le_bytes([*x0, *x1]);
                    let y = i16::from_le_bytes([*y0, *y1]);
                    (f32::from(x) * unit, f32::from(y) * unit)
                })
                .collect();
            if target == 0 {
                drawing.silhouette.push(points);
            } else {
                drawing.lines.push(points);
            }
        }
    }
    Some(drawing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05
    }

    /// A curve an editor draws is followed along its bend, not cut straight to its end point.
    #[test]
    fn a_hand_drawn_curve_is_followed_along_its_bend() {
        let drawing = parse(
            r#"<svg><g id="lines"><g id="base"><path d="M0 0 C0 10 10 10 10 0"/></g></g></svg>"#,
        );
        let line = &drawing.lines[0];
        assert!(close(line[0], (0.0, 0.0)));
        assert!(close(*line.last().unwrap(), (10.0, 0.0)));
        let peak = line.iter().map(|point| point.1).fold(f32::MIN, f32::max);
        assert!(
            (peak - 7.5).abs() < 0.1,
            "a cubic through (0,10) and (10,10) peaks at 7.5: {peak}"
        );
        assert_eq!(line.len(), CURVE_STEPS + 1);
    }

    /// A half circle drawn as an arc keeps its radius all the way round.
    #[test]
    fn an_arc_is_followed_at_its_radius() {
        let drawing = parse(r#"<svg><g id="lines"><path d="M-10 0 A10 10 0 0 1 10 0"/></g></svg>"#);
        let line = &drawing.lines[0];
        assert!(close(*line.last().unwrap(), (10.0, 0.0)));
        assert!(
            line.iter()
                .all(|(x, y)| ((x * x + y * y).sqrt() - 10.0).abs() < 0.05)
        );
        assert!(line.len() > 4);
    }

    /// Editors move and scale groups and shapes with `transform`; the drawing lands where they put it.
    #[test]
    fn transforms_on_groups_and_shapes_are_applied() {
        let drawing = parse(
            r#"<svg><g id="silhouette" transform="translate(100 0)"><rect x="0" y="0" width="10" height="5" transform="scale(2)"/></g>
            <g id="lines"><g transform="rotate(90)"><line x1="0" y1="0" x2="10" y2="0"/></g></g></svg>"#,
        );
        let ring = &drawing.silhouette[0];
        assert!(close(ring[0], (100.0, 0.0)));
        assert!(close(ring[2], (120.0, 10.0)));
        let line = &drawing.lines[0];
        assert!(close(line[1], (0.0, 10.0)), "{line:?}");
        let circles = parse(r#"<svg><g id="lines"><circle cx="5" cy="5" r="2"/></g></svg>"#);
        assert!(
            circles.lines[0]
                .iter()
                .all(|(x, y)| (((x - 5.0).powi(2) + (y - 5.0).powi(2)).sqrt() - 2.0).abs() < 0.01)
        );
    }

    /// The straight commands the generator writes still read exactly.
    #[test]
    fn generated_drawings_still_read_exactly() {
        let drawing = parse(
            r#"<svg><g id="silhouette"><path fill-rule="evenodd" d="M0 0 L10 0 L10 10 Z M2 2 h3 v3 Z"/></g><g id="origin"><path d="M-3 0 H3"/></g></svg>"#,
        );
        assert_eq!(drawing.silhouette.len(), 2);
        assert_eq!(
            drawing.silhouette[0],
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)]
        );
        assert_eq!(
            drawing.silhouette[1],
            vec![(2.0, 2.0), (5.0, 2.0), (5.0, 5.0), (2.0, 2.0)]
        );
        assert!(drawing.lines.is_empty());
    }
}
