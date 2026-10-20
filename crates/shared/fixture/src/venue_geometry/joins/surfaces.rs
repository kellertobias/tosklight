use super::{Wall, deform_x, math::*};
// Poses originate in f32 native/render matrices. At venue scale their rigid round-trip
// introduces micrometres of error; these must not turn a shared cap into an exposed surface.
const PLANE_EPS: f64 = 0.01;
#[derive(Clone)]
pub(super) struct Solid {
    planes: Vec<([f64; 3], f64)>,
}
impl Solid {
    fn hull(points: &[[f64; 3]]) -> Self {
        let mut planes: Vec<([f64; 3], f64)> = Vec::new();
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                for k in j + 1..points.len() {
                    let mut n = normal([points[i], points[j], points[k]]);
                    if length(n) < 0.5 {
                        continue;
                    }
                    let mut d = -dot(n, points[i]);
                    let distances: Vec<_> = points.iter().map(|&p| dot(n, p) + d).collect();
                    if distances.iter().all(|&v| v >= -1e-5) {
                        n = mul(n, -1.0);
                        d = -d;
                    } else if !distances.iter().all(|&v| v <= 1e-5) {
                        continue;
                    }
                    if !planes.iter().any(|(other, offset)| {
                        dot(*other, n) > 0.999999 && (*offset - d).abs() < 1e-4
                    }) {
                        planes.push((n, d));
                    }
                }
            }
        }
        Self { planes }
    }
    pub(super) fn same_surface(&self, triangle: &[[f64; 3]; 3], normal: [f64; 3]) -> bool {
        self.planes.iter().any(|&(n, d)| {
            dot(n, normal) > 0.999999 && triangle.iter().all(|&p| (dot(n, p) + d).abs() < PLANE_EPS)
        })
    }
    pub(super) fn subtract(&self, mut inside: Vec<[f64; 3]>) -> Vec<Vec<[f64; 3]>> {
        let mut outside = Vec::new();
        for &(n, d) in &self.planes {
            if inside.is_empty() {
                break;
            }
            let part = clip(&inside, n, d, false);
            if part.len() > 2 {
                outside.push(part);
            }
            inside = clip(&inside, n, d, true);
        }
        outside
    }
    pub(super) fn subtract_line(&self, line: [[f64; 3]; 2]) -> Vec<[[f64; 3]; 2]> {
        let mut entry = 0.0_f64;
        let mut exit = 1.0_f64;
        for &(n, d) in &self.planes {
            let a = dot(n, line[0]) + d;
            let b = dot(n, line[1]) + d;
            if a > PLANE_EPS && b > PLANE_EPS {
                return vec![line];
            }
            if (a > PLANE_EPS) != (b > PLANE_EPS) {
                let t = a / (a - b);
                if a > b {
                    entry = entry.max(t)
                } else {
                    exit = exit.min(t)
                }
            }
        }
        if exit < entry {
            return vec![line];
        }
        let at = |t| add(line[0], mul(sub(line[1], line[0]), t));
        let mut result = Vec::new();
        if entry > 1e-7 {
            result.push([line[0], at(entry)]);
        }
        if exit < 1.0 - 1e-7 {
            result.push([at(exit), line[1]]);
        }
        result
    }
    pub(super) fn same_plan_side(&self, line: [[f64; 3]; 2], centre: [f64; 3]) -> bool {
        let midpoint = mul(add(line[0], line[1]), 0.5);
        let tangent = sub(line[1], line[0]);
        let mut outward = [-tangent[2], 0.0, tangent[0]];
        if dot(outward, sub(centre, midpoint)) > 0.0 {
            outward = mul(outward, -1.0);
        }
        let l = length(outward);
        if l < 1e-8 {
            return false;
        }
        outward = mul(outward, 1.0 / l);
        self.planes.iter().any(|&(n, d)| {
            dot(n, outward) > 0.999999
                && (dot(n, line[0]) + d).abs() < PLANE_EPS
                && (dot(n, line[1]) + d).abs() < PLANE_EPS
        })
    }
}
fn clip(poly: &[[f64; 3]], n: [f64; 3], d: f64, inside: bool) -> Vec<[f64; 3]> {
    if !inside && poly.iter().all(|&p| dot(n, p) + d <= PLANE_EPS) {
        return Vec::new();
    }
    let sign = if inside { 1.0 } else { -1.0 };
    let mut result = Vec::new();
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let da = (dot(n, a) + d) * sign;
        let db = (dot(n, b) + d) * sign;
        if da <= PLANE_EPS {
            result.push(a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            result.push(add(a, mul(sub(b, a), da / (da - db))));
        }
    }
    result.dedup_by(|a, b| distance(*a, *b) < 1e-6);
    if result.len() > 2 && distance(result[0], *result.last().unwrap()) < 1e-6 {
        result.pop();
    }
    result
}
pub(super) fn solids(wall: &Wall) -> Vec<Solid> {
    wall.geometry
        .triangles
        .chunks_exact(12)
        .filter_map(|chunk| {
            let mut vertices = Vec::new();
            for point in chunk.iter().flatten() {
                let p = transform(wall.source.local_to_world, *point);
                if !vertices.iter().any(|&v| distance(v, p) < 1e-6) {
                    vertices.push(p);
                }
            }
            let solid = Solid::hull(&vertices);
            (solid.planes.len() >= 4).then_some(solid)
        })
        .collect()
}
pub(super) fn plan_solids(wall: &Wall) -> Vec<Solid> {
    let s = wall.source.size_millimetres;
    let mut cuts = vec![-s[0] / 2.0, s[0] / 2.0];
    for o in &wall.source.openings {
        cuts.extend([
            f64::from(o.offset_millimetres) - s[0] / 2.0,
            f64::from(o.offset_millimetres + o.width_millimetres) - s[0] / 2.0,
        ]);
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    cuts.windows(2)
        .filter_map(|p| {
            let x = (p[0] + p[1]) / 2.0;
            if wall.source.openings.iter().any(|o| {
                x > f64::from(o.offset_millimetres) - s[0] / 2.0
                    && x < f64::from(o.offset_millimetres + o.width_millimetres) - s[0] / 2.0
            }) {
                return None;
            }
            let mut points = Vec::new();
            for x in p {
                for z in [-s[2] / 2.0, s[2] / 2.0] {
                    for y in [-1e8, 1e8] {
                        points.push(transform(
                            wall.source.local_to_world,
                            [deform_x(*x, z, s, wall.ends), y, z],
                        ));
                    }
                }
            }
            Some(Solid::hull(&points))
        })
        .collect()
}
