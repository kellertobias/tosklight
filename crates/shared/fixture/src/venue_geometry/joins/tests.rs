use super::*;
fn wall(
    id: u128,
    x: f64,
    z: f64,
    yaw: f64,
    length: f64,
    thickness: f64,
    height: f64,
) -> VenueWallPlacement {
    let (s, c) = yaw.sin_cos();
    VenueWallPlacement {
        id: Uuid::from_u128(id),
        size_millimetres: [length, height, thickness],
        local_to_world: [
            [c, 0.0, -s, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [s, 0.0, c, 0.0],
            [x, height / 2.0, z, 1.0],
        ],
        openings: vec![],
    }
}
#[test]
fn right_angle_corner_uses_one_continuous_perimeter_without_endcaps() {
    let walls = [
        wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
        wall(
            2,
            0.0,
            1000.0,
            -std::f64::consts::FRAC_PI_2,
            2000.0,
            200.0,
            3000.0,
        ),
    ];
    let result = venue_joined_walls(&walls);
    assert_eq!(result.len(), 2);
    let plan: Vec<_> = walls
        .iter()
        .flat_map(|w| {
            result[&w.id]
                .plan
                .iter()
                .map(|line| line.map(|p| transform(w.local_to_world, [p[0], 0.0, p[1]])))
        })
        .collect();
    let perimeter: f64 = plan.iter().map(|p| distance(p[0], p[1])).sum();
    assert!((perimeter - 8400.0).abs() < 0.1, "perimeter {perimeter}");
    for geometry in result.values() {
        assert!(geometry.triangles.iter().all(|t| length(normal(*t)) > 0.5));
    }
}
#[test]
fn collinear_join_removes_both_shared_caps_and_keeps_exposed_height_step() {
    let walls = [
        wall(1, -1000.0, 0.0, 0.0, 2000.0, 400.0, 4000.0),
        wall(2, 1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
    ];
    let joined = venue_joined_walls(&walls);
    assert_eq!(joined.len(), 2);
    let cap_area: f64 = joined[&walls[0].id]
        .triangles
        .iter()
        .filter(|t| t.iter().all(|p| (p[0] - 1000.0).abs() < 1e-5))
        .map(|t| length(math::cross(sub(t[1], t[0]), sub(t[2], t[0]))) / 2.0)
        .sum();
    assert!(
        (cap_area - 1_000_000.0).abs() < 0.1,
        "exposed cap area {cap_area}"
    );
}
#[test]
fn intentional_gap_is_not_silently_welded() {
    let walls = [
        wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
        wall(2, 1005.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
    ];
    assert!(venue_joined_walls(&walls).is_empty());
}

fn hit(origin: [f64; 3], direction: [f64; 3], triangle: [[f64; 3]; 3]) -> bool {
    let e1 = sub(triangle[1], triangle[0]);
    let e2 = sub(triangle[2], triangle[0]);
    let p = math::cross(direction, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-8 {
        return false;
    }
    let offset = sub(origin, triangle[0]);
    let u = dot(offset, p) / det;
    let q = math::cross(offset, e1);
    let v = dot(direction, q) / det;
    u >= -1e-7 && v >= -1e-7 && u + v <= 1.0 + 1e-7 && dot(e2, q) / det > 1e-6
}
fn blocked(
    walls: &[VenueWallPlacement],
    joined: &BTreeMap<Uuid, VenueGeometry>,
    origin: [f64; 3],
    direction: [f64; 3],
) -> bool {
    walls.iter().any(|wall| {
        joined[&wall.id].triangles.iter().any(|t| {
            hit(
                origin,
                direction,
                t.map(|p| transform(wall.local_to_world, p)),
            )
        })
    })
}
#[test]
fn rotated_reversed_and_oblique_corners_have_closed_contours_and_block_diagonal_light() {
    for rotation in [0.0, 0.7, std::f64::consts::FRAC_PI_2, std::f64::consts::PI] {
        for angle in [std::f64::consts::FRAC_PI_4, std::f64::consts::FRAC_PI_2] {
            for reversed in [false, true] {
                let mut walls = [
                    wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
                    wall(
                        2,
                        1000.0 * angle.cos(),
                        1000.0 * angle.sin(),
                        -angle,
                        2000.0,
                        300.0,
                        3000.0,
                    ),
                ];
                if reversed {
                    let a = &mut walls[0];
                    a.local_to_world[0][0] = -1.0;
                    a.local_to_world[2][2] = -1.0;
                }
                let (s, c) = rotation.sin_cos();
                let rotate = |p: [f64; 3]| [c * p[0] - s * p[2], p[1], s * p[0] + c * p[2]];
                for wall in &mut walls {
                    for column in &mut wall.local_to_world {
                        let p = rotate([column[0], column[1], column[2]]);
                        column[..3].copy_from_slice(&p);
                    }
                }
                let joined = venue_joined_walls(&walls);
                assert_eq!(joined.len(), 2);
                let mut points = Vec::new();
                for wall in &walls {
                    for line in &joined[&wall.id].plan {
                        for p in line {
                            points.push(transform(wall.local_to_world, [p[0], 0.0, p[1]]));
                        }
                    }
                }
                for p in &points {
                    assert_eq!(
                        points.iter().filter(|q| distance(*p, **q) < 0.01).count(),
                        2,
                        "open or doubled perimeter at {p:?}, angle {angle}, reversed {reversed}"
                    );
                }
                assert!(blocked(
                    &walls,
                    &joined,
                    rotate([40.0, 1500.0, -1000.0]),
                    rotate([0.0, 0.0, 1.0])
                ));
                assert!(
                    joined
                        .values()
                        .all(|g| g.triangles.iter().all(|t| length(normal(*t)) > 0.5))
                );
            }
        }
    }
}
#[test]
fn tee_and_partial_height_corner_keep_exposed_caps_without_filling_windows() {
    use crate::{VenueWindowShape, WallOpeningKind};
    let mut a = wall(1, 0.0, 0.0, 0.0, 4000.0, 200.0, 4000.0);
    a.openings.push(WallOpening {
        id: Uuid::from_u128(90),
        offset_millimetres: 500,
        sill_millimetres: 1000,
        width_millimetres: 1000,
        height_millimetres: 1000,
        opening: WallOpeningKind::Window {
            shape: VenueWindowShape::Round,
        },
    });
    let walls = [
        a,
        wall(
            2,
            0.0,
            1000.0,
            -std::f64::consts::FRAC_PI_2,
            2000.0,
            200.0,
            3000.0,
        ),
    ];
    let joined = venue_joined_walls(&walls);
    assert!(
        !blocked(&walls, &joined, [-1000.0, 1500.0, -1000.0], [0.0, 0.0, 1.0]),
        "round window remains physically open"
    );
    assert!(blocked(
        &walls,
        &joined,
        [500.0, 1500.0, -1000.0],
        [0.0, 0.0, 1.0]
    ));
    let tee_cap: f64 = joined[&walls[1].id]
        .triangles
        .iter()
        .filter(|t| t.iter().all(|p| (p[0] + 1000.0).abs() < 1e-5))
        .map(|t| length(math::cross(sub(t[1], t[0]), sub(t[2], t[0]))) / 2.0)
        .sum();
    assert!(tee_cap < 0.01, "buried tee end cap removed: {tee_cap}");
    let partial = [
        wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 4000.0),
        wall(
            2,
            0.0,
            1000.0,
            -std::f64::consts::FRAC_PI_2,
            2000.0,
            200.0,
            3000.0,
        ),
    ];
    let joined = venue_joined_walls(&partial);
    assert!(
        joined[&partial[0].id]
            .triangles
            .iter()
            .flatten()
            .all(|p| p[0] <= 1000.01),
        "upper exposed wall does not inherit lower wall miter"
    );
}
#[test]
fn representative_twenty_wall_initial_join_is_bounded() {
    let walls: Vec<_> = (0..20)
        .map(|i| wall(i + 1, i as f64 * 2000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0))
        .collect();
    let start = std::time::Instant::now();
    let joined = venue_joined_walls(&walls);
    eprintln!(
        "20 wall initial join: {:?}, {} triangles",
        start.elapsed(),
        joined.values().map(|g| g.triangles.len()).sum::<usize>()
    );
    assert_eq!(joined.len(), 20);
    assert!(joined.values().map(|g| g.triangles.len()).sum::<usize>() < 400);
}

#[test]
fn endpoint_door_keeps_its_authored_aperture_and_hinge() {
    use crate::{VenueDoorHinge, VenueDoorLeaves, VenueDoorSwing, WallOpeningKind};
    let mut a = wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0);
    a.openings.push(WallOpening {
        id: Uuid::from_u128(100),
        offset_millimetres: 1000,
        sill_millimetres: 0,
        width_millimetres: 1000,
        height_millimetres: 2100,
        opening: WallOpeningKind::Door {
            leaves: VenueDoorLeaves::Single,
            swing: VenueDoorSwing::Inward,
            hinge: VenueDoorHinge::Left,
        },
    });
    let walls = [
        a,
        wall(
            2,
            0.0,
            1000.0,
            -std::f64::consts::FRAC_PI_2,
            2000.0,
            200.0,
            3000.0,
        ),
    ];
    let joined = venue_joined_walls(&walls);
    assert!(!blocked(
        &walls,
        &joined,
        [-500.0, 1000.0, -1000.0],
        [0.0, 0.0, 1.0]
    ));
    assert!(
        joined[&walls[0].id]
            .triangles
            .iter()
            .flatten()
            .all(|p| p[0] <= 1000.01),
        "endpoint jamb must not be extended"
    );
}

#[test]
fn rounded_collinear_endpoints_share_one_terminal_plane() {
    let walls = [
        wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
        wall(2, 1001.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
    ];
    let joined = venue_joined_walls(&walls);
    assert!(
        blocked(&walls, &joined, [0.5, 1500.0, -1000.0], [0.0, 0.0, 1.0]),
        "rounded endpoint must not leak"
    );
    let perimeter: f64 = joined
        .values()
        .flat_map(|g| g.plan.iter())
        .map(|line| ((line[1][0] - line[0][0]).powi(2) + (line[1][1] - line[0][1]).powi(2)).sqrt())
        .sum();
    assert!(
        (perimeter - 8402.0).abs() < 0.01,
        "no internal caps: {perimeter}"
    );
}

fn world_edges(
    walls: &[VenueWallPlacement],
    meshes: &BTreeMap<Uuid, VenueGeometry>,
) -> Vec<[[f64; 3]; 2]> {
    walls
        .iter()
        .flat_map(|w| {
            meshes[&w.id]
                .exterior
                .iter()
                .map(|line| line.map(|p| transform(w.local_to_world, p)))
        })
        .collect()
}
#[test]
fn exterior_edges_remove_collinear_ownership_seams_but_keep_real_corner_creases() {
    let walls = [
        wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
        wall(2, 1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
    ];
    let joined = venue_joined_walls(&walls);
    let edges = world_edges(&walls, &joined);
    assert!(
        edges
            .iter()
            .all(|line| !line.iter().all(|p| p[0].abs() < 0.01)),
        "no ownership cap or vertical silhouette seam: {edges:?}"
    );
    let total: f64 = edges.iter().map(|line| distance(line[0], line[1])).sum();
    assert!(
        (total - 28800.0).abs() < 0.1,
        "solid cuboid exterior12edge length: {total}"
    );
    let walls = [
        wall(1, -1000.0, 0.0, 0.0, 2000.0, 200.0, 3000.0),
        wall(
            2,
            0.0,
            1000.0,
            -std::f64::consts::FRAC_PI_2,
            2000.0,
            200.0,
            3000.0,
        ),
    ];
    let edges = world_edges(&walls, &venue_joined_walls(&walls));
    assert!(
        edges
            .iter()
            .any(|line| (line[0][1] - line[1][1]).abs() > 2999.0
                && (line[0][0] - 100.0).abs() < 0.01
                && (line[0][2] + 100.0).abs() < 0.01),
        "real outer corner remains"
    );
}
#[test]
fn exterior_edges_suppress_nonconforming_round_window_strip_seams_and_buried_tee_caps() {
    use crate::{VenueWindowShape, WallOpeningKind};
    let mut a = wall(1, 0.0, 0.0, 0.0, 4000.0, 200.0, 3000.0);
    a.openings.push(WallOpening {
        id: Uuid::from_u128(50),
        offset_millimetres: 500,
        sill_millimetres: 1000,
        width_millimetres: 1000,
        height_millimetres: 1000,
        opening: WallOpeningKind::Window {
            shape: VenueWindowShape::Round,
        },
    });
    let walls = [
        a,
        wall(
            2,
            0.0,
            1000.0,
            -std::f64::consts::FRAC_PI_2,
            2000.0,
            200.0,
            3000.0,
        ),
    ];
    let joined = venue_joined_walls(&walls);
    let edges = world_edges(&walls, &joined);
    // The top strip has many tessellated cuts; none may extend vertically above the window.
    assert!(
        edges.iter().all(|line| !(line[0][0] > -1499.0
            && line[0][0] < -501.0
            && (line[0][0] - line[1][0]).abs() < 0.01
            && line.iter().all(|p| p[1] > 2000.01)
            && (line[0][1] - line[1][1]).abs() > 0.01)),
        "window triangulation must not produce false strip edges"
    );
    assert!(
        edges
            .iter()
            .all(|line| !(line.iter().all(|p| p[2].abs() < 0.01)
                && (line[0][1] - line[1][1]).abs() > 0.01
                && line[0][0].abs() < 99.0)),
        "buried tee cap has no vertical edge"
    );
    assert!(
        joined[&walls[0].id].elevation.len() > 4,
        "semantic window frames retained"
    );
}
