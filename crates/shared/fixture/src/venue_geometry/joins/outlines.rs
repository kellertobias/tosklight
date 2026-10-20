//! Physical edge extraction across wall ownership boundaries, including nonconforming aperture strips.
use super::*;
#[derive(Clone)]
struct Face {
    id: Uuid,
    triangle: [[f64; 3]; 3],
    low: [f64; 3],
    high: [f64; 3],
}
struct Plane {
    normal: [f64; 3],
    offset: f64,
    faces: Vec<Face>,
}
const EPS: f64 = 0.001;
pub(super) fn apply(walls: &[Wall], meshes: &mut BTreeMap<Uuid, VenueGeometry>) {
    let mut planes: Vec<Plane> = Vec::new();
    for wall in walls {
        let Some(mesh) = meshes.get(&wall.source.id) else {
            continue;
        };
        for triangle in &mesh.triangles {
            let triangle = triangle.map(|p| transform(wall.source.local_to_world, p));
            let n = normal(triangle);
            let d = -dot(n, triangle[0]);
            let index = planes
                .iter()
                .position(|p| dot(p.normal, n) > 0.999999 && (p.offset - d).abs() < EPS)
                .unwrap_or_else(|| {
                    planes.push(Plane {
                        normal: n,
                        offset: d,
                        faces: vec![],
                    });
                    planes.len() - 1
                });
            let low = std::array::from_fn(|i| {
                triangle.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min)
            });
            let high = std::array::from_fn(|i| {
                triangle
                    .iter()
                    .map(|p| p[i])
                    .fold(f64::NEG_INFINITY, f64::max)
            });
            planes[index].faces.push(Face {
                id: wall.source.id,
                triangle,
                low,
                high,
            });
        }
    }
    let mut edges: BTreeMap<[[i64; 3]; 2], (Uuid, [[f64; 3]; 2])> = BTreeMap::new();
    for plane in planes {
        for face in &plane.faces {
            for edge in [
                [face.triangle[0], face.triangle[1]],
                [face.triangle[1], face.triangle[2]],
                [face.triangle[2], face.triangle[0]],
            ] {
                for segment in boundary(edge, &plane) {
                    let mut key = segment.map(|p| p.map(|v| (v / EPS).round() as i64));
                    key.sort();
                    let entry = edges.entry(key).or_insert((face.id, segment));
                    if face.id < entry.0 {
                        *entry = (face.id, segment);
                    }
                }
            }
        }
    }
    for (id, segment) in edges.into_values() {
        let pose = walls
            .iter()
            .find(|w| w.source.id == id)
            .unwrap()
            .source
            .local_to_world;
        meshes
            .get_mut(&id)
            .unwrap()
            .exterior
            .push(segment.map(|p| inverse(pose, p)));
    }
}
fn boundary(edge: [[f64; 3]; 2], plane: &Plane) -> Vec<[[f64; 3]; 2]> {
    let delta = sub(edge[1], edge[0]);
    let squared = dot(delta, delta);
    if squared < EPS * EPS {
        return vec![];
    }
    let candidates: Vec<_> = plane
        .faces
        .iter()
        .filter(|f| {
            (0..3).all(|i| {
                f.low[i] <= edge[0][i].max(edge[1][i]) + EPS
                    && f.high[i] >= edge[0][i].min(edge[1][i]) - EPS
            })
        })
        .collect();
    let mut cuts = vec![0.0, 1.0];
    for face in &candidates {
        for &point in &face.triangle {
            let t = dot(sub(point, edge[0]), delta) / squared;
            if t > 0.0 && t < 1.0 && distance(point, add(edge[0], mul(delta, t))) < EPS {
                cuts.push(t);
            }
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() * squared.sqrt() < EPS);
    let inward = math::cross(plane.normal, mul(delta, 1.0 / squared.sqrt()));
    cuts.windows(2)
        .filter_map(|pair| {
            let midpoint = add(edge[0], mul(delta, (pair[0] + pair[1]) / 2.0));
            let mut positive = false;
            let mut negative = false;
            for face in &candidates {
                if !contains(face.triangle, midpoint, plane.normal) {
                    continue;
                }
                for &vertex in &face.triangle {
                    let side = dot(sub(vertex, midpoint), inward);
                    positive |= side > EPS;
                    negative |= side < -EPS;
                }
                if positive && negative {
                    return None;
                }
            }
            Some([pair[0], pair[1]].map(|t| add(edge[0], mul(delta, t))))
        })
        .collect()
}
fn contains(triangle: [[f64; 3]; 3], p: [f64; 3], n: [f64; 3]) -> bool {
    (0..3).all(|i| {
        dot(
            math::cross(sub(triangle[(i + 1) % 3], triangle[i]), sub(p, triangle[i])),
            n,
        ) >= -EPS
    })
}
