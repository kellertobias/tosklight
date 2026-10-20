use super::*;
fn pose(x: f64, y: f64, z: f64, yaw: f64) -> [[f64; 4]; 4] {
    let (s, c) = yaw.sin_cos();
    [
        [c, 0.0, -s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [s, 0.0, c, 0.0],
        [x, y, z, 1.0],
    ]
}
fn slab(elevation: i32) -> VenueArchitecture {
    VenueArchitecture::Slab {
        purpose: VenueSlabPurpose::Balcony,
        outline_millimetres: vec![
            VenuePlanPoint { x: -5000, y: -5000 },
            VenuePlanPoint { x: 5000, y: -5000 },
            VenuePlanPoint { x: 5000, y: 5000 },
            VenuePlanPoint { x: -5000, y: 5000 },
        ],
        elevation_millimetres: elevation,
        thickness_millimetres: 200,
    }
}
fn top_area(g: &VenueGeometry, height: f64) -> f64 {
    g.triangles
        .iter()
        .filter(|t| t.iter().all(|p| (p[1] - height).abs() < 1e-6))
        .map(|t| cross([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]).abs() / 2.0)
        .sum()
}
#[test]
fn stairs_have_real_individual_treads_and_an_unambiguous_up_direction() {
    let g = venue_geometry(
        &VenueArchitecture::Stairs { steps: 12 },
        [1200.0, 3000.0, 4800.0],
    )
    .unwrap();
    assert_eq!(g.triangles.len(), 12 * 12);
    let top: Vec<_> = g
        .triangles
        .iter()
        .filter(|t| t.iter().all(|p| p[1] == 1500.0))
        .collect();
    assert!(!top.is_empty());
    assert!(
        top.iter()
            .flat_map(|triangle| triangle.iter())
            .all(|p| p[2] >= 2000.0)
    );
    assert!(g.plan.contains(&[[0.0, -1680.0], [0.0, 1680.0]]));
}
#[test]
fn rotated_stair_cutout_removes_both_slab_faces_and_builds_reveals() {
    let stairs = venue_stair_volume([1200.0, 3000.0, 4500.0], pose(800.0, 1500.0, -400.0, 0.6));
    let g = venue_geometry_in_context(
        &slab(3000),
        [10000.0, 200.0, 10000.0],
        pose(0.0, 0.0, 0.0, 0.0),
        &[stairs],
    )
    .unwrap();
    assert!((top_area(&g, 3200.0) - (100_000_000.0 - 1200.0 * 4500.0)).abs() < 0.01);
    assert!((top_area(&g, 3000.0) - (100_000_000.0 - 1200.0 * 4500.0)).abs() < 0.01);
    assert!(
        g.triangles
            .iter()
            .any(|t| t.iter().any(|p| p[1] == 3000.0) && t.iter().any(|p| p[1] == 3200.0))
    );
}
#[test]
fn supporting_and_unreached_floors_stay_whole_but_multiple_reached_levels_cut() {
    let stairs = venue_stair_volume([1200.0, 3000.0, 4500.0], pose(0.0, 1500.0, 0.0, 0.0));
    for elevation in [-200, 0, 1000, 2000, 3000, 4000] {
        let g = venue_geometry_in_context(
            &slab(elevation),
            [10000.0, 200.0, 10000.0],
            pose(0.0, 0.0, 0.0, 0.0),
            std::slice::from_ref(&stairs),
        )
        .unwrap();
        let expected = if elevation > 0 && elevation <= 3000 {
            94_600_000.0
        } else {
            100_000_000.0
        };
        assert!(
            (top_area(&g, f64::from(elevation) + 200.0) - expected).abs() < 0.01,
            "elevation {elevation}"
        );
    }
}
#[test]
fn overlapping_stair_apertures_subtract_their_union_only_once() {
    let a = venue_stair_volume([1000.0, 3000.0, 4000.0], pose(0.0, 1500.0, 0.0, 0.0));
    let b = venue_stair_volume([1000.0, 3000.0, 4000.0], pose(500.0, 1500.0, 0.0, 0.0));
    let g = venue_geometry_in_context(
        &slab(2000),
        [10000.0, 200.0, 10000.0],
        pose(0.0, 0.0, 0.0, 0.0),
        &[a, b],
    )
    .unwrap();
    assert!((top_area(&g, 2200.0) - 94_000_000.0).abs() < 0.01);
}

fn perimeter(g: &VenueGeometry) -> f64 {
    g.plan
        .iter()
        .map(|line| (line[0][0] - line[1][0]).hypot(line[0][1] - line[1][1]))
        .sum()
}
#[test]
fn slab_plan_contains_boundary_contours_without_internal_subdivision_lines() {
    let floor_pose = pose(0.0, 0.0, 0.0, 0.0);
    let hole = venue_stair_volume([1200.0, 3000.0, 4500.0], pose(800.0, 1500.0, -400.0, 0.6));
    let geometry =
        venue_geometry_in_context(&slab(2000), [10000.0, 200.0, 10000.0], floor_pose, &[hole])
            .unwrap();
    assert!(geometry.has_stair_cutouts);
    assert!((perimeter(&geometry) - 51_400.0).abs() < 0.01);
    let edge_hole = venue_stair_volume([1200.0, 3000.0, 4500.0], pose(4900.0, 1500.0, 0.0, 0.0));
    let geometry = venue_geometry_in_context(
        &slab(2000),
        [10000.0, 200.0, 10000.0],
        floor_pose,
        &[edge_hole],
    )
    .unwrap();
    assert!((perimeter(&geometry) - 41_400.0).abs() < 0.01);
}
#[test]
fn legacy_floor_adapter_preserves_its_physical_box_and_only_reports_real_intersections() {
    let floor = venue_floor_slab([10000.0, 200.0, 10000.0]);
    let stairs = venue_stair_volume([1200.0, 3000.0, 4500.0], pose(0.0, 1500.0, 0.0, 0.0));
    let base = venue_geometry_in_context(
        &floor,
        [10000.0, 200.0, 10000.0],
        pose(0.0, -100.0, 0.0, 0.0),
        std::slice::from_ref(&stairs),
    )
    .unwrap();
    assert!(!base.has_stair_cutouts);
    assert_eq!(base.triangles.len(), 12);
    let raised = venue_geometry_in_context(
        &floor,
        [10000.0, 200.0, 10000.0],
        pose(0.0, 2100.0, 0.0, 0.0),
        std::slice::from_ref(&stairs),
    )
    .unwrap();
    assert!(raised.has_stair_cutouts);
    let outside = venue_geometry_in_context(
        &floor,
        [10000.0, 200.0, 10000.0],
        pose(20000.0, 2100.0, 0.0, 0.0),
        &[stairs],
    )
    .unwrap();
    assert!(!outside.has_stair_cutouts);
    assert_eq!(outside.triangles.len(), 12);
}
