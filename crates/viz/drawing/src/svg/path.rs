//! SVG path data, flattened into polylines.
//!
//! Every command of the `d` grammar that draws is handled: lines, cubic and quadratic Béziers with
//! their smooth forms, and elliptical arcs. Curves are subdivided by their control-polygon length,
//! so a long sweeping curve is cut more finely than a short one and both stay near the tolerance.

use super::numbers;

/// Subpath under construction.
struct Pen {
    points: Vec<[f64; 2]>,
    start: [f64; 2],
    cursor: [f64; 2],
    /// Reflection point for a smooth cubic or quadratic continuation.
    previous_control: Option<([f64; 2], bool)>,
}

impl Pen {
    fn new() -> Self {
        Self {
            points: Vec::new(),
            start: [0.0, 0.0],
            cursor: [0.0, 0.0],
            previous_control: None,
        }
    }

    fn push(&mut self, point: [f64; 2]) {
        if self.points.last().is_none_or(|last| {
            (last[0] - point[0]).abs() > 1e-12 || (last[1] - point[1]).abs() > 1e-12
        }) {
            self.points.push(point);
        }
        self.cursor = point;
    }
}

fn tokens(data: &str) -> Vec<(char, Vec<f64>)> {
    let mut commands: Vec<(char, Vec<f64>)> = Vec::new();
    let mut letter: Option<char> = None;
    let mut start = 0;
    for (index, character) in data.char_indices() {
        if !character.is_ascii_alphabetic() {
            continue;
        }
        if let Some(command) = letter {
            commands.push((command, numbers(&data[start..index])));
        }
        letter = Some(character);
        start = index + character.len_utf8();
    }
    if let Some(command) = letter {
        commands.push((command, numbers(&data[start..])));
    }
    commands
}

/// How many straight segments a curve of this control-polygon length needs.
fn steps(length: f64, tolerance: f64) -> usize {
    ((length / tolerance.max(1e-6)).sqrt().ceil() as usize).clamp(2, 256)
}

fn cubic(from: [f64; 2], c1: [f64; 2], c2: [f64; 2], to: [f64; 2], tolerance: f64, pen: &mut Pen) {
    let length = distance(from, c1) + distance(c1, c2) + distance(c2, to);
    let count = steps(length, tolerance);
    for step in 1..=count {
        let t = step as f64 / count as f64;
        let inverse = 1.0 - t;
        pen.push([
            inverse.powi(3) * from[0]
                + 3.0 * inverse.powi(2) * t * c1[0]
                + 3.0 * inverse * t * t * c2[0]
                + t.powi(3) * to[0],
            inverse.powi(3) * from[1]
                + 3.0 * inverse.powi(2) * t * c1[1]
                + 3.0 * inverse * t * t * c2[1]
                + t.powi(3) * to[1],
        ]);
    }
}

fn quadratic(from: [f64; 2], control: [f64; 2], to: [f64; 2], tolerance: f64, pen: &mut Pen) {
    let length = distance(from, control) + distance(control, to);
    let count = steps(length, tolerance);
    for step in 1..=count {
        let t = step as f64 / count as f64;
        let inverse = 1.0 - t;
        pen.push([
            inverse * inverse * from[0] + 2.0 * inverse * t * control[0] + t * t * to[0],
            inverse * inverse * from[1] + 2.0 * inverse * t * control[1] + t * t * to[1],
        ]);
    }
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

/// The endpoint parameterisation of `A`, turned into the centre form the flattener needs.
#[allow(clippy::too_many_arguments)]
fn elliptical_arc(
    from: [f64; 2],
    mut rx: f64,
    mut ry: f64,
    rotation_degrees: f64,
    large: bool,
    sweep_flag: bool,
    to: [f64; 2],
    tolerance: f64,
    pen: &mut Pen,
) {
    if rx.abs() < 1e-12 || ry.abs() < 1e-12 {
        pen.push(to);
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    let (sin, cos) = rotation_degrees.to_radians().sin_cos();
    let dx = (from[0] - to[0]) / 2.0;
    let dy = (from[1] - to[1]) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let oversize = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if oversize > 1.0 {
        let growth = oversize.sqrt();
        rx *= growth;
        ry *= growth;
    }
    let numerator = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let factor = if denominator <= 0.0 {
        0.0
    } else {
        (numerator / denominator).sqrt() * if large == sweep_flag { -1.0 } else { 1.0 }
    };
    let cx1 = factor * rx * y1 / ry;
    let cy1 = -factor * ry * x1 / rx;
    let centre = [
        cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2.0,
        sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2.0,
    ];
    let angle = |x: f64, y: f64| ((y - cy1) / ry).atan2((x - cx1) / rx);
    let start = angle(x1, y1);
    let end = angle(-x1, -y1);
    let mut sweep = end - start;
    if sweep_flag && sweep < 0.0 {
        sweep += std::f64::consts::TAU;
    } else if !sweep_flag && sweep > 0.0 {
        sweep -= std::f64::consts::TAU;
    }
    let count = steps(rx.max(ry) * sweep.abs(), tolerance);
    for step in 1..=count {
        let t = start + sweep * (step as f64 / count as f64);
        let x = rx * t.cos();
        let y = ry * t.sin();
        pen.push([centre[0] + cos * x - sin * y, centre[1] + sin * x + cos * y]);
    }
}

/// Flatten one `d` attribute into its subpaths and whether each was closed.
pub(crate) fn path_polylines(data: &str, tolerance: f64) -> Vec<(Vec<[f64; 2]>, bool)> {
    let mut output: Vec<(Vec<[f64; 2]>, bool)> = Vec::new();
    let mut pen = Pen::new();
    let finish = |pen: &mut Pen, closed: bool, output: &mut Vec<(Vec<[f64; 2]>, bool)>| {
        if pen.points.len() >= 2 {
            output.push((std::mem::take(&mut pen.points), closed));
        } else {
            pen.points.clear();
        }
    };

    for (command, values) in tokens(data) {
        let relative = command.is_ascii_lowercase();
        let absolute = |pen: &Pen, x: f64, y: f64| {
            if relative {
                [pen.cursor[0] + x, pen.cursor[1] + y]
            } else {
                [x, y]
            }
        };
        match command.to_ascii_uppercase() {
            'M' => {
                for (index, pair) in values.as_chunks::<2>().0.iter().enumerate() {
                    let point = absolute(&pen, pair[0], pair[1]);
                    if index == 0 {
                        finish(&mut pen, false, &mut output);
                        pen.start = point;
                        pen.cursor = point;
                        pen.points.push(point);
                    } else {
                        pen.push(point);
                    }
                }
                pen.previous_control = None;
            }
            'L' => {
                for pair in values.as_chunks::<2>().0 {
                    let point = absolute(&pen, pair[0], pair[1]);
                    pen.push(point);
                }
                pen.previous_control = None;
            }
            'H' => {
                for value in &values {
                    let x = if relative {
                        pen.cursor[0] + value
                    } else {
                        *value
                    };
                    pen.push([x, pen.cursor[1]]);
                }
                pen.previous_control = None;
            }
            'V' => {
                for value in &values {
                    let y = if relative {
                        pen.cursor[1] + value
                    } else {
                        *value
                    };
                    pen.push([pen.cursor[0], y]);
                }
                pen.previous_control = None;
            }
            'C' => {
                for chunk in values.as_chunks::<6>().0 {
                    let from = pen.cursor;
                    let c1 = absolute(&pen, chunk[0], chunk[1]);
                    let c2 = absolute(&pen, chunk[2], chunk[3]);
                    let to = absolute(&pen, chunk[4], chunk[5]);
                    cubic(from, c1, c2, to, tolerance, &mut pen);
                    pen.previous_control = Some((c2, true));
                }
            }
            'S' => {
                for chunk in values.as_chunks::<4>().0 {
                    let from = pen.cursor;
                    let c1 = match pen.previous_control {
                        Some((control, true)) => {
                            [2.0 * from[0] - control[0], 2.0 * from[1] - control[1]]
                        }
                        _ => from,
                    };
                    let c2 = absolute(&pen, chunk[0], chunk[1]);
                    let to = absolute(&pen, chunk[2], chunk[3]);
                    cubic(from, c1, c2, to, tolerance, &mut pen);
                    pen.previous_control = Some((c2, true));
                }
            }
            'Q' => {
                for chunk in values.as_chunks::<4>().0 {
                    let from = pen.cursor;
                    let control = absolute(&pen, chunk[0], chunk[1]);
                    let to = absolute(&pen, chunk[2], chunk[3]);
                    quadratic(from, control, to, tolerance, &mut pen);
                    pen.previous_control = Some((control, false));
                }
            }
            'T' => {
                for chunk in values.as_chunks::<2>().0 {
                    let from = pen.cursor;
                    let control = match pen.previous_control {
                        Some((control, false)) => {
                            [2.0 * from[0] - control[0], 2.0 * from[1] - control[1]]
                        }
                        _ => from,
                    };
                    let to = absolute(&pen, chunk[0], chunk[1]);
                    quadratic(from, control, to, tolerance, &mut pen);
                    pen.previous_control = Some((control, false));
                }
            }
            'A' => {
                for chunk in values.as_chunks::<7>().0 {
                    let from = pen.cursor;
                    let to = absolute(&pen, chunk[5], chunk[6]);
                    elliptical_arc(
                        from,
                        chunk[0],
                        chunk[1],
                        chunk[2],
                        chunk[3] != 0.0,
                        chunk[4] != 0.0,
                        to,
                        tolerance,
                        &mut pen,
                    );
                    pen.cursor = to;
                }
                pen.previous_control = None;
            }
            'Z' => {
                let start = pen.start;
                finish(&mut pen, true, &mut output);
                pen.cursor = start;
                pen.previous_control = None;
            }
            _ => {}
        }
    }
    finish(&mut pen, false, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_absolute_and_relative_lines() {
        let subpaths = path_polylines("M 0 0 L 100 0 l 0 100 Z", 0.5);
        assert_eq!(subpaths.len(), 1);
        let (points, closed) = &subpaths[0];
        assert!(closed);
        assert_eq!(points, &vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]]);
    }

    #[test]
    fn horizontal_and_vertical_commands_track_the_cursor() {
        let (points, _) = path_polylines("M 10 10 H 50 V 30", 0.5).remove(0);
        assert_eq!(points, vec![[10.0, 10.0], [50.0, 10.0], [50.0, 30.0]]);
    }

    #[test]
    fn a_cubic_curve_is_flattened_between_its_endpoints() {
        let (points, _) = path_polylines("M 0 0 C 0 100 100 100 100 0", 0.5).remove(0);
        assert_eq!(points.first(), Some(&[0.0, 0.0]));
        assert_eq!(points.last(), Some(&[100.0, 0.0]));
        assert!(points.len() > 4, "{points:?}");
        assert!(
            points
                .iter()
                .all(|point| point[1] >= -1e-9 && point[1] <= 75.1)
        );
    }

    #[test]
    fn a_half_circle_arc_reaches_its_endpoint() {
        let (points, _) = path_polylines("M 0 0 A 50 50 0 0 1 100 0", 0.5).remove(0);
        let last = points.last().expect("an endpoint");
        assert!((last[0] - 100.0).abs() < 1e-6, "{last:?}");
        assert!(last[1].abs() < 1e-6, "{last:?}");
        // A sweep flag of 1 runs through increasing angles of SVG's Y-down space, so the arc
        // bulges towards negative Y — upwards on screen — by its radius.
        let extreme = points.iter().map(|point| point[1]).fold(f64::MAX, f64::min);
        assert!((extreme + 50.0).abs() < 1.0, "{extreme}");
    }

    #[test]
    fn several_subpaths_come_back_separately() {
        let subpaths = path_polylines("M 0 0 L 10 0 M 20 0 L 30 0", 0.5);
        assert_eq!(subpaths.len(), 2);
        assert_eq!(subpaths[1].0[0], [20.0, 0.0]);
    }
}
