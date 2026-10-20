use super::*;
#[test]
fn concave_slab_has_complete_top_area_in_both_windings() {
    let points = vec![
        [0, 0],
        [4000, 0],
        [4000, 1000],
        [1000, 1000],
        [1000, 4000],
        [0, 4000],
    ];
    for reverse in [false, true] {
        let mut outline: Vec<_> = points
            .iter()
            .map(|p| VenuePlanPoint { x: p[0], y: p[1] })
            .collect();
        if reverse {
            outline.reverse();
        }
        let g = venue_geometry(
            &VenueArchitecture::Slab {
                purpose: VenueSlabPurpose::Balcony,
                outline_millimetres: outline,
                elevation_millimetres: 2000,
                thickness_millimetres: 200,
            },
            [4000.0, 200.0, 4000.0],
        )
        .unwrap();
        let area: f64 = g
            .triangles
            .iter()
            .filter(|t| t.iter().all(|p| p[1] == 2200.0))
            .map(|t| cross([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]).abs() / 2.0)
            .sum();
        assert!((area - 7_000_000.0).abs() < 0.1);
    }
}
#[test]
fn circular_and_arched_windows_remove_physical_wall_surface() {
    for shape in [
        VenueWindowShape::Rectangular,
        VenueWindowShape::Round,
        VenueWindowShape::Arched,
    ] {
        let g = venue_geometry(
            &VenueArchitecture::Wall {
                openings: vec![WallOpening {
                    id: uuid::Uuid::new_v4(),
                    offset_millimetres: 1000,
                    sill_millimetres: 1000,
                    width_millimetres: 1000,
                    height_millimetres: 1000,
                    opening: WallOpeningKind::Window { shape },
                }],
                ceiling_height_millimetres: None,
            },
            [3000.0, 3000.0, 200.0],
        )
        .unwrap();
        for t in g
            .triangles
            .iter()
            .filter(|t| t.iter().all(|p| p[2] == 100.0))
        {
            let a = t[0];
            let b = t[1];
            let c = t[2];
            let p = [0.0, 0.0];
            let sign = [
                cross([a[0], a[1]], [b[0], b[1]], p),
                cross([b[0], b[1]], [c[0], c[1]], p),
                cross([c[0], c[1]], [a[0], a[1]], p),
            ];
            assert!(!sign.iter().all(|v| *v > 0.0001) && !sign.iter().all(|v| *v < -0.0001));
        }
    }
}
#[test]
fn furniture_styles_are_real_open_geometry_and_keep_custom_sizes() {
    let mut counts = Vec::new();
    for style in [
        VenueChairStyle::BlackMetal,
        VenueChairStyle::White,
        VenueChairStyle::BarStool,
    ] {
        let g =
            venue_geometry(&VenueArchitecture::Chair { style }, [500.0, 1000.0, 500.0]).unwrap();
        assert!(g.triangles.len() > 12);
        assert_eq!(g.metallic.len(), g.triangles.len());
        assert_eq!(
            g.metallic.iter().any(|metal| *metal),
            style != VenueChairStyle::White
        );
        counts.push(g.triangles.len());
    }
    assert_ne!(counts[0], counts[1]);
    assert_ne!(counts[1], counts[2]);
    let bar = venue_geometry(&VenueArchitecture::Bar, [4000.0, 1100.0, 600.0]).unwrap();
    assert_eq!(bar.triangles.len(), 12);
}

#[test]
fn connected_room_ceiling_is_deterministic_when_walls_are_reordered() {
    let mut walls: Vec<_> = [
        ([0.0, 0.0], [4000.0, 0.0]),
        ([4000.0, 0.0], [4000.0, 3000.0]),
        ([4000.0, 3000.0], [0.0, 3000.0]),
        ([0.0, 3000.0], [0.0, 0.0]),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (start, end))| VenueWallRun {
        id: uuid::Uuid::from_u128(i as u128 + 1),
        start_millimetres: start,
        end_millimetres: end,
        floor_millimetres: 0.0,
        height_millimetres: 4000.0,
        ceiling_height_millimetres: Some(if i == 0 { 3500 } else { 3000 }),
    })
    .collect();
    assert_eq!(venue_rooms(&walls)[0].height_millimetres, 3000.0);
    walls.reverse();
    assert_eq!(venue_rooms(&walls)[0].height_millimetres, 3000.0);
}

#[test]
fn open_door_leaves_keep_physical_swing_hinges_and_custom_dimensions() {
    let size = [4000.0, 3000.0, 200.0];
    let make = |kind| VenueArchitecture::Wall {
        openings: vec![WallOpening {
            id: uuid::Uuid::new_v4(),
            offset_millimetres: 1000,
            sill_millimetres: 0,
            width_millimetres: 1600,
            height_millimetres: 2100,
            opening: kind,
        }],
        ceiling_height_millimetres: None,
    };
    let aperture = venue_geometry(
        &make(WallOpeningKind::Window {
            shape: VenueWindowShape::Rectangular,
        }),
        size,
    )
    .unwrap();
    for leaves in [VenueDoorLeaves::Single, VenueDoorLeaves::Double] {
        for swing in [VenueDoorSwing::Inward, VenueDoorSwing::Outward] {
            for hinge in [VenueDoorHinge::Left, VenueDoorHinge::Right] {
                let geometry = venue_geometry(
                    &make(WallOpeningKind::Door {
                        leaves,
                        swing,
                        hinge,
                    }),
                    size,
                )
                .unwrap();
                let count = if leaves == VenueDoorLeaves::Double {
                    2
                } else {
                    1
                };
                assert_eq!(
                    geometry.triangles.len(),
                    aperture.triangles.len() + count * 12
                );
                let leaf_vertices: Vec<_> = geometry.triangles[aperture.triangles.len()..]
                    .iter()
                    .flatten()
                    .collect();
                let radius = 1600.0 / count as f64;
                let z_bounds = leaf_vertices
                    .iter()
                    .fold([f64::INFINITY, f64::NEG_INFINITY], |[min, max], p| {
                        [min.min(p[2]), max.max(p[2])]
                    });
                assert_eq!(
                    z_bounds,
                    if swing == VenueDoorSwing::Inward {
                        [0.0, radius]
                    } else {
                        [-radius, 0.0]
                    }
                );
                assert!(
                    leaf_vertices
                        .iter()
                        .all(|p| p[1] >= -1500.0 && p[1] <= 600.0)
                );
                let hinge_positions: &[f64] = if count == 2 {
                    &[-1000.0, 600.0]
                } else if hinge == VenueDoorHinge::Left {
                    &[-1000.0]
                } else {
                    &[600.0]
                };
                assert!(
                    leaf_vertices
                        .iter()
                        .all(|p| hinge_positions.iter().any(|x| (p[0] - x).abs() == 17.5))
                );
                for x in hinge_positions {
                    assert!(geometry.plan.contains(&[
                        [*x, 0.0],
                        [
                            *x,
                            if swing == VenueDoorSwing::Inward {
                                radius
                            } else {
                                -radius
                            }
                        ]
                    ]));
                }
            }
        }
    }
}
