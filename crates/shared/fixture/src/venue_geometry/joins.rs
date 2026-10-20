//! Derived wall junctions. Authored dimensions, opening identities and placement remain untouched.
use super::*;
use std::collections::BTreeMap;
use uuid::Uuid;
mod math;
mod outlines;
mod surfaces;
use math::*;

#[derive(Clone, Debug, PartialEq)]
pub struct VenueWallPlacement {
    pub id: Uuid,
    pub size_millimetres: [f64; 3],
    /// Rigid column-major pose of the centred wall, in millimetres.
    pub local_to_world: [[f64; 4]; 4],
    pub openings: Vec<WallOpening>,
}
#[derive(Clone)]
struct Wall {
    source: VenueWallPlacement,
    geometry: VenueGeometry,
    ends: [[f64; 2]; 2],
    neighbours: Vec<usize>,
}

/// Returned geometry remains local to each physical wall. Only connected walls are included;
/// callers retain ordinary geometry for isolated walls, and apply each wall's own material.
pub fn venue_joined_walls(walls: &[VenueWallPlacement]) -> BTreeMap<Uuid, VenueGeometry> {
    let mut walls = prepare(walls);
    for i in 0..walls.len() {
        for j in i + 1..walls.len() {
            if connected(&walls[i].source, &walls[j].source) {
                walls[i].neighbours.push(j);
                walls[j].neighbours.push(i);
            }
        }
    }
    for i in 0..walls.len() {
        for end in 0..2 {
            let joint_point = endpoint(&walls[i].source, end);
            let joined: Vec<_> = walls[i]
                .neighbours
                .iter()
                .filter_map(|&j| {
                    (0..2)
                        .find(|&other| {
                            distance(joint_point, endpoint(&walls[j].source, other))
                                <= JOIN_TOLERANCE
                        })
                        .map(|other| (j, other))
                })
                .collect();
            if joined.len() == 1 {
                let (j, other) = joined[0];
                if let Some(miter) = miter(&walls[i].source, end, &walls[j].source, other) {
                    walls[i].ends[end] = miter;
                }
            }
        }
    }
    for wall in &mut walls {
        if wall.neighbours.is_empty() {
            continue;
        }
        let size = wall.source.size_millimetres;
        let ends = wall.ends;
        for point in wall.geometry.triangles[..wall.geometry.wall_body_triangles]
            .iter_mut()
            .flatten()
        {
            point[0] = deform_x(point[0], point[2], size, ends);
        }
        for line in &mut wall.geometry.plan[wall.geometry.wall_plan_symbols..] {
            for point in line {
                point[0] = deform_x(point[0], point[1], size, ends);
            }
        }
    }
    let solids: Vec<_> = walls.iter().map(surfaces::solids).collect();
    let plans: Vec<_> = walls.iter().map(surfaces::plan_solids).collect();
    let mut result = BTreeMap::new();
    for wall in &walls {
        if wall.neighbours.is_empty() {
            continue;
        }
        let mut geometry = VenueGeometry {
            joined_wall: true,
            elevation: crate::venue_wall_drawing(
                wall.source.size_millimetres,
                &wall.source.openings,
            )
            .into_iter()
            .skip(4)
            .collect(),
            ..Default::default()
        };
        for triangle in &wall.geometry.triangles {
            let world = triangle.map(|p| transform(wall.source.local_to_world, p));
            let face_normal = normal(world);
            if length(face_normal) < 0.5 {
                continue;
            }
            let mut pieces = vec![world.to_vec()];
            for &other in &wall.neighbours {
                for solid in &solids[other] {
                    if wall.source.id < walls[other].source.id
                        && solid.same_surface(&world, face_normal)
                    {
                        continue;
                    }
                    pieces = pieces
                        .into_iter()
                        .flat_map(|piece| solid.subtract(piece))
                        .collect();
                }
            }
            for polygon in pieces {
                for i in 1..polygon.len() - 1 {
                    let triangle = [polygon[0], polygon[i], polygon[i + 1]]
                        .map(|p| inverse(wall.source.local_to_world, p));
                    if length(normal(triangle)) > 0.5 {
                        geometry.triangle(triangle[0], triangle[1], triangle[2]);
                    }
                }
            }
        }
        for (line_index, line) in wall.geometry.plan.iter().enumerate() {
            if line_index < wall.geometry.wall_plan_symbols {
                geometry.plan.push(*line);
                continue;
            }
            let world = line.map(|p| transform(wall.source.local_to_world, [p[0], 0.0, p[1]]));
            let mut pieces = vec![world];
            for &other in &wall.neighbours {
                for solid in &plans[other] {
                    let centre = transform(wall.source.local_to_world, [0.0; 3]);
                    if wall.source.id < walls[other].source.id
                        && solid.same_plan_side(world, centre)
                    {
                        continue;
                    }
                    pieces = pieces
                        .into_iter()
                        .flat_map(|line| solid.subtract_line(line))
                        .collect();
                }
            }
            for segment in pieces {
                geometry.plan.push(segment.map(|p| {
                    let local = inverse(wall.source.local_to_world, p);
                    [local[0], local[2]]
                }));
            }
        }
        result.insert(wall.source.id, geometry);
    }
    outlines::apply(&walls, &mut result);
    result
}
fn prepare(walls: &[VenueWallPlacement]) -> Vec<Wall> {
    walls
        .iter()
        .filter_map(|source| {
            if !upright(source.local_to_world) {
                return None;
            }
            let geometry = venue_geometry(
                &VenueArchitecture::Wall {
                    openings: source.openings.clone(),
                    ceiling_height_millimetres: None,
                },
                source.size_millimetres,
            )
            .ok()?;
            let half = source.size_millimetres[0] / 2.0;
            Some(Wall {
                source: source.clone(),
                geometry,
                ends: [[-half, -half], [half, half]],
                neighbours: Vec::new(),
            })
        })
        .collect()
}
const JOIN_TOLERANCE: f64 = 1.5;
fn upright(pose: [[f64; 4]; 4]) -> bool {
    pose.iter().flatten().all(|v| v.is_finite())
        && pose[1][1] > 0.99999
        && pose[0][1].abs() < 1e-5
        && pose[2][1].abs() < 1e-5
}
fn endpoint(w: &VenueWallPlacement, end: usize) -> [f64; 3] {
    transform(
        w.local_to_world,
        [
            if end == 0 { -1.0 } else { 1.0 } * w.size_millimetres[0] / 2.0,
            0.0,
            0.0,
        ],
    )
}
fn connected(a: &VenueWallPlacement, b: &VenueWallPlacement) -> bool {
    let overlap = (a.local_to_world[3][1] + a.size_millimetres[1] / 2.0)
        .min(b.local_to_world[3][1] + b.size_millimetres[1] / 2.0)
        - (a.local_to_world[3][1] - a.size_millimetres[1] / 2.0)
            .max(b.local_to_world[3][1] - b.size_millimetres[1] / 2.0);
    if overlap <= 0.01 {
        return false;
    }
    let a0 = endpoint(a, 0);
    let a1 = endpoint(a, 1);
    let b0 = endpoint(b, 0);
    let b1 = endpoint(b, 1);
    [a0, a1]
        .iter()
        .any(|&p| plan_distance(p, b0, b1) <= JOIN_TOLERANCE)
        || [b0, b1]
            .iter()
            .any(|&p| plan_distance(p, a0, a1) <= JOIN_TOLERANCE)
}
fn miter(
    a: &VenueWallPlacement,
    end: usize,
    b: &VenueWallPlacement,
    other: usize,
) -> Option<[f64; 2]> {
    // An aperture ending at a junction keeps its authored jamb and door hinge exactly in place.
    if opening_at_end(a, end) || opening_at_end(b, other) {
        return None;
    }
    // A partial-height neighbour must not drag the exposed upper wall into its miter.
    if (a.local_to_world[3][1] - b.local_to_world[3][1]).abs() > 0.01
        || (a.size_millimetres[1] - b.size_millimetres[1]).abs() > 0.01
    {
        return None;
    }
    let direction_a = mul(axis(a.local_to_world, 0), if end == 0 { -1.0 } else { 1.0 });
    let direction_b = mul(
        axis(b.local_to_world, 0),
        if other == 0 { 1.0 } else { -1.0 },
    );
    let denominator = cross2(direction_a, direction_b);
    if denominator.abs() < 1e-6 {
        // Millimetre-rounded collinear endpoints share the same terminal plane.
        let midpoint = mul(add(endpoint(a, end), endpoint(b, other)), 0.5);
        let x = inverse(a.local_to_world, midpoint)[0];
        return Some([x, x]);
    }
    if denominator.abs() < 0.2588 {
        return None;
    }
    let mut values = [0.0; 2];
    let limit =
        (a.size_millimetres[2].max(b.size_millimetres[2]) * 4.0).min(a.size_millimetres[0] * 0.45);
    for (index, side) in [-1.0, 1.0].into_iter().enumerate() {
        let a_side = add(
            endpoint(a, end),
            mul(
                axis(a.local_to_world, 2),
                side * a.size_millimetres[2] / 2.0,
            ),
        );
        let b_side = add(
            endpoint(b, other),
            mul(
                axis(b.local_to_world, 2),
                side * if end == 0 { -1.0 } else { 1.0 }
                    * if other == 0 { 1.0 } else { -1.0 }
                    * b.size_millimetres[2]
                    / 2.0,
            ),
        );
        let distance = cross2(sub(b_side, a_side), direction_b) / denominator;
        let local = inverse(a.local_to_world, add(a_side, mul(direction_a, distance)));
        if (local[0]
            - if end == 0 {
                -a.size_millimetres[0] / 2.0
            } else {
                a.size_millimetres[0] / 2.0
            })
        .abs()
            > limit
        {
            return None;
        }
        values[index] = local[0];
    }
    Some(values)
}
fn opening_at_end(wall: &VenueWallPlacement, end: usize) -> bool {
    wall.openings.iter().any(|opening| {
        if end == 0 {
            opening.offset_millimetres == 0
        } else {
            (f64::from(opening.offset_millimetres) + f64::from(opening.width_millimetres)
                - wall.size_millimetres[0])
                .abs()
                < 0.01
        }
    })
}
fn deform_x(x: f64, z: f64, size: [f64; 3], ends: [[f64; 2]; 2]) -> f64 {
    let end = if (x + size[0] / 2.0).abs() < 1e-5 {
        0
    } else if (x - size[0] / 2.0).abs() < 1e-5 {
        1
    } else {
        return x;
    };
    ends[end][0] + (ends[end][1] - ends[end][0]) * (z / size[2] + 0.5)
}
#[cfg(test)]
mod tests;
