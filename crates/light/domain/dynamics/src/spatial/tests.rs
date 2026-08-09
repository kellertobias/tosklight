use super::*;
use uuid::Uuid;

fn fixture(value: u128) -> FixtureId {
    FixtureId(Uuid::from_u128(value))
}

fn target(value: u128, x: f64, y: f64, z: f64) -> SpatialTarget {
    SpatialTarget {
        fixture_id: fixture(value),
        position: Some(Position3d { x, y, z }),
    }
}

fn mapping(preset: ProjectionPreset, shape: SpatialSelectionShape) -> SpatialSelectionMapping {
    SpatialSelectionMapping {
        projection: SpatialProjection::from_preset(preset, Position3d::default()),
        shape,
    }
}

/// A cylinder's direction is its axis, so world +Z stands it up; a sphere's is the centre of
/// its spread, so world +X centres it there. Those are the two frames the tests measure in.
fn centered(kind: ProjectionKind, anchor: Position3d) -> SpatialProjection {
    directed(
        kind,
        anchor,
        match kind {
            ProjectionKind::Spherical => UNIT_X,
            _ => UNIT_Z,
        },
    )
}

fn directed(
    kind: ProjectionKind,
    anchor: Position3d,
    view_direction: Vector3,
) -> SpatialProjection {
    SpatialProjection {
        anchor,
        view_direction,
        rotation_degrees: 0.0,
        preset: None,
        kind,
    }
}

fn coordinates(projection: &SpatialProjection, x: f64, y: f64, z: f64) -> (f64, f64) {
    let projected = project_spatial_positions(projection, &[target(1, x, y, z)]).unwrap();
    (projected[0].u.unwrap(), projected[0].v.unwrap())
}

#[test]
fn a_stored_projection_without_a_kind_stays_planar() {
    // Shows saved before the cylindrical and spherical kinds existed carry no `kind`.
    let stored = serde_json::json!({
        "anchor": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "view_direction": { "x": 0.0, "y": 0.0, "z": -1.0 },
        "rotation_degrees": 0.0,
        "preset": "top",
    });
    let projection: SpatialProjection = serde_json::from_value(stored).unwrap();
    assert_eq!(projection.kind, ProjectionKind::Planar);
    assert_eq!(projection.preset, Some(ProjectionPreset::Top));
    assert_eq!(projection.rotation_degrees, 0.0);
    assert_eq!(
        projection.view_direction,
        ProjectionPreset::Top.view_direction()
    );
}

#[test]
fn a_planar_projection_serializes_exactly_what_it_did_before_the_new_kinds() {
    // Persisted Shows must not gain fields, so a stored planar projection has to round-trip
    // byte for byte.
    let stored = serde_json::json!({
        "anchor": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "view_direction": { "x": 0.0, "y": 0.0, "z": -1.0 },
        "rotation_degrees": 0.0,
        "preset": "top",
    });
    let projection: SpatialProjection = serde_json::from_value(stored.clone()).unwrap();
    assert_eq!(serde_json::to_value(&projection).unwrap(), stored);
}

#[test]
fn a_non_planar_projection_persists_only_the_shared_fields() {
    let mut projection = centered(ProjectionKind::Cylindrical, Position3d::default());
    projection.rotation_degrees = 45.0;
    let encoded = serde_json::to_value(&projection).unwrap();
    assert_eq!(encoded["kind"], "cylindrical");
    assert_eq!(encoded["rotation_degrees"], 45.0);
    // The angles the angular kinds used to carry are gone, not merely defaulted.
    assert!(encoded.get("axis_rotation").is_none());
    assert!(encoded.get("start_angle_degrees").is_none());
    assert!(encoded.get("elevation_degrees").is_none());
    assert_eq!(
        serde_json::from_value::<SpatialProjection>(encoded).unwrap(),
        projection
    );
}

#[test]
fn a_stored_cylindrical_projection_keeps_its_spread_after_the_angles_became_a_direction() {
    // Written when a cylinder carried Euler angles and a start angle of its own: a quarter
    // turn about X lays the axis onto world -Y, and the spread starts 30 degrees round.
    let stored = serde_json::json!({
        "anchor": { "x": 1.0, "y": 2.0, "z": 3.0 },
        "view_direction": { "x": 0.0, "y": 0.0, "z": -1.0 },
        "rotation_degrees": 0.0,
        "kind": "cylindrical",
        "axis_rotation": { "x": 90.0, "y": 0.0, "z": 0.0 },
        "start_angle_degrees": 30.0,
    });
    let migrated: SpatialProjection = serde_json::from_value(stored).unwrap();

    assert_eq!(migrated.kind, ProjectionKind::Cylindrical);
    // The axis is now the direction itself.
    assert!((migrated.view_direction.y.abs() - 1.0).abs() < 1.0e-9);
    assert!(migrated.view_direction.x.abs() < 1.0e-9);
    assert!(migrated.view_direction.z.abs() < 1.0e-9);
    // And it ranks every fixture exactly where it used to: the spread is still centred 30
    // degrees off world +X, so +X reads 30 and the 30-degree mark itself reads 0.
    assert!((coordinates(&migrated, 4.0, 2.0, 3.0).0 - 30.0).abs() < 1.0e-9);
    let (sin, cos) = 30f64.to_radians().sin_cos();
    assert!(
        coordinates(&migrated, 1.0 + 3.0 * cos, 2.0, 3.0 + 3.0 * sin)
            .0
            .abs()
            < 1.0e-9
    );
    // Round-tripping it again writes none of the legacy angles back.
    let encoded = serde_json::to_value(&migrated).unwrap();
    assert!(encoded.get("axis_rotation").is_none());
    assert!(encoded.get("start_angle_degrees").is_none());
    assert_eq!(
        serde_json::from_value::<SpatialProjection>(encoded).unwrap(),
        migrated
    );
}

#[test]
fn a_stored_angular_projection_with_no_direction_keeps_the_frame_it_had() {
    // The angular kinds ignored the direction, so one could be saved without a usable one.
    for (kind, x, y, z) in [("cylindrical", 0.0, 0.0, 3.0), ("spherical", 3.0, 0.0, 0.0)] {
        let stored = serde_json::json!({
            "anchor": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "view_direction": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "rotation_degrees": 0.0,
            "kind": kind,
        });
        let migrated: SpatialProjection = serde_json::from_value(stored).unwrap();
        // A cylinder stands up and a sphere centres on world +X, as both used to.
        assert_eq!(coordinates(&migrated, x, y, z).0, 0.0);
    }
}

#[test]
fn a_stored_spherical_projection_keeps_its_centre_after_the_angles_became_a_direction() {
    let stored = serde_json::json!({
        "anchor": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "view_direction": { "x": 0.0, "y": 0.0, "z": -1.0 },
        "rotation_degrees": 0.0,
        "kind": "spherical",
        "start_angle_degrees": 90.0,
        "elevation_degrees": 0.0,
    });
    let migrated: SpatialProjection = serde_json::from_value(stored).unwrap();

    assert_eq!(migrated.kind, ProjectionKind::Spherical);
    // Azimuth 90 with no elevation centred the spread on world +Y, and still does.
    assert!(coordinates(&migrated, 0.0, 3.0, 0.0).0.abs() < 1.0e-9);
    assert!((coordinates(&migrated, 0.0, -3.0, 0.0).0 - 180.0).abs() < 1.0e-9);
}

#[test]
fn a_cylindrical_projection_spreads_outward_from_the_start_angle() {
    let projection = centered(ProjectionKind::Cylindrical, Position3d::default());

    // With every rotation at zero the axis is world +Z and the start angle sits on +X.
    assert_eq!(coordinates(&projection, 3.0, 0.0, 0.0).0, 0.0);
    // The two sides of the start angle are equidistant, which is what "outward both
    // ways" means: +Y and -Y both read 90 degrees rather than 90 and 270.
    assert!((coordinates(&projection, 0.0, 3.0, 0.0).0 - 90.0).abs() < 1.0e-9);
    assert!((coordinates(&projection, 0.0, -3.0, 0.0).0 - 90.0).abs() < 1.0e-9);
    // They meet 180 degrees away, on the far side of the cylinder.
    assert!((coordinates(&projection, -3.0, 0.0, 0.0).0 - 180.0).abs() < 1.0e-9);
    // Distance from the axis does not change the angle; `v` carries the axial position.
    assert!((coordinates(&projection, 0.0, 40.0, 7.0).0 - 90.0).abs() < 1.0e-9);
    assert!((coordinates(&projection, 0.0, 40.0, 7.0).1 - 7.0).abs() < 1.0e-9);
}

#[test]
fn the_cylindrical_start_angle_moves_the_center_of_the_spread() {
    let mut projection = centered(ProjectionKind::Cylindrical, Position3d::default());
    projection.rotation_degrees = 90.0;

    // The centre has moved onto +Y, so +X and -X are now the equidistant pair.
    assert!(coordinates(&projection, 0.0, 3.0, 0.0).0.abs() < 1.0e-9);
    assert!((coordinates(&projection, 3.0, 0.0, 0.0).0 - 90.0).abs() < 1.0e-9);
    assert!((coordinates(&projection, -3.0, 0.0, 0.0).0 - 90.0).abs() < 1.0e-9);
    assert!((coordinates(&projection, 0.0, -3.0, 0.0).0 - 180.0).abs() < 1.0e-9);
}

#[test]
fn pointing_the_cylinder_axis_elsewhere_reorients_the_spread() {
    // The axis is simply the direction, so laying it onto world -Y is one field.
    let projection = directed(
        ProjectionKind::Cylindrical,
        Position3d::default(),
        Vector3 {
            x: 0.0,
            y: -1.0,
            z: 0.0,
        },
    );

    // The axis now runs along Y, so a Y offset is axial and reads on `v`, not on `u`.
    let (u, v) = coordinates(&projection, 0.0, 5.0, 0.0);
    assert!(u.abs() < 1.0e-9);
    assert!((v.abs() - 5.0).abs() < 1.0e-9);
    // The start direction stays on +X, so +X is still the centre of the spread.
    assert!(coordinates(&projection, 4.0, 0.0, 0.0).0.abs() < 1.0e-9);
    assert!((coordinates(&projection, -4.0, 0.0, 0.0).0 - 180.0).abs() < 1.0e-9);
}

#[test]
fn the_cylinder_anchor_is_freely_positionable() {
    let projection = centered(
        ProjectionKind::Cylindrical,
        Position3d {
            x: 10.0,
            y: -4.0,
            z: 2.0,
        },
    );

    // Measured from the anchor, not from the world origin.
    assert_eq!(coordinates(&projection, 13.0, -4.0, 2.0).0, 0.0);
    assert!((coordinates(&projection, 7.0, -4.0, 2.0).0 - 180.0).abs() < 1.0e-9);
    assert!((coordinates(&projection, 10.0, -4.0, 9.0).1 - 7.0).abs() < 1.0e-9);
}

#[test]
fn a_spherical_projection_spreads_outward_to_the_antipode() {
    let projection = centered(ProjectionKind::Spherical, Position3d::default());

    // Both angles at zero put the centre on +X.
    assert_eq!(coordinates(&projection, 3.0, 0.0, 0.0).0, 0.0);
    // Every direction 90 degrees off the centre reads the same, in both axes.
    for (x, y, z) in [
        (0.0, 3.0, 0.0),
        (0.0, -3.0, 0.0),
        (0.0, 0.0, 3.0),
        (0.0, 0.0, -3.0),
    ] {
        assert!((coordinates(&projection, x, y, z).0 - 90.0).abs() < 1.0e-9);
    }
    // 180 degrees at the antipode.
    assert!((coordinates(&projection, -3.0, 0.0, 0.0).0 - 180.0).abs() < 1.0e-9);
    // `u` is the angle alone, so distance along the centre direction does not move it.
    assert!(coordinates(&projection, 50.0, 0.0, 0.0).0.abs() < 1.0e-9);
}

#[test]
fn a_spherical_rotation_turns_the_meridian_the_spread_is_measured_from() {
    // Centre on +X puts the start meridian on +Z, the same reference the cylinder uses.
    let projection = centered(ProjectionKind::Spherical, Position3d::default());
    assert!(coordinates(&projection, 0.0, 0.0, 3.0).1.abs() < 1.0e-9);
    assert!((coordinates(&projection, 0.0, -3.0, 0.0).1 - 90.0).abs() < 1.0e-9);
    assert!((coordinates(&projection, 0.0, 3.0, 0.0).1 + 90.0).abs() < 1.0e-9);

    // Rolling the start carries every meridian with it, by the whole roll.
    let rolled = SpatialProjection {
        rotation_degrees: 90.0,
        ..projection
    };
    assert!((coordinates(&rolled, 0.0, 0.0, 3.0).1 + 90.0).abs() < 1.0e-9);
    assert!((coordinates(&rolled, 0.0, -3.0, 0.0).1).abs() < 1.0e-9);

    // The angle from the centre is a roll invariant, so it stays put.
    assert_eq!(
        coordinates(&projection, 0.0, 3.0, 0.0).0,
        coordinates(&rolled, 0.0, 3.0, 0.0).0
    );
}

#[test]
fn the_spherical_direction_moves_the_center_in_both_axes() {
    // What used to be an azimuth and an elevation is the direction to the centre.
    let sideways = directed(ProjectionKind::Spherical, Position3d::default(), UNIT_Y);
    assert!(coordinates(&sideways, 0.0, 3.0, 0.0).0.abs() < 1.0e-9);
    assert!((coordinates(&sideways, 0.0, -3.0, 0.0).0 - 180.0).abs() < 1.0e-9);

    let overhead = directed(ProjectionKind::Spherical, Position3d::default(), UNIT_Z);
    assert!(coordinates(&overhead, 0.0, 0.0, 3.0).0.abs() < 1.0e-9);
    assert!((coordinates(&overhead, 0.0, 0.0, -3.0).0 - 180.0).abs() < 1.0e-9);
    assert!((coordinates(&overhead, 3.0, 0.0, 0.0).0 - 90.0).abs() < 1.0e-9);
}

#[test]
fn every_kind_needs_the_direction_that_orients_it() {
    // One direction places all three kinds now, so none of them can do without it.
    for kind in [
        ProjectionKind::Planar,
        ProjectionKind::Cylindrical,
        ProjectionKind::Spherical,
    ] {
        let projection = directed(kind, Position3d::default(), Vector3::default());
        assert!(matches!(
            project_spatial_positions(&projection, &[target(1, 1.0, 1.0, 1.0)]),
            Err(SpatialMappingError::InvalidViewDirection)
        ));
    }
}

#[test]
fn a_fixture_on_the_anchor_itself_stays_rankable() {
    for kind in [ProjectionKind::Cylindrical, ProjectionKind::Spherical] {
        let projection = centered(kind, Position3d::default());
        assert_eq!(coordinates(&projection, 0.0, 0.0, 0.0), (0.0, 0.0));
    }
}

#[test]
fn named_projection_presets_follow_the_stage_xyz_convention() {
    let bottom = SpatialProjection::from_preset(ProjectionPreset::Bottom, Position3d::default());
    assert_eq!(
        bottom.view_direction,
        Vector3 {
            x: 0.0,
            y: 0.0,
            z: 1.0
        }
    );

    let targets = [
        target(1, -2.0, 0.0, 0.0),
        target(2, 2.0, 0.0, 0.0),
        target(3, 0.0, 2.0, 0.0),
        target(4, 0.0, -2.0, 0.0),
        target(5, 0.0, 0.0, 2.0),
        target(6, 0.0, 0.0, -2.0),
    ];
    let ascending_x = SpatialSelectionShape::Grid {
        angle_degrees: 0.0,
        direction: RankDirection::Ascending,
    };
    let ascending_up = SpatialSelectionShape::Grid {
        angle_degrees: 90.0,
        direction: RankDirection::Ascending,
    };

    let top = evaluate_spatial_mapping(
        &mapping(ProjectionPreset::Top, ascending_up.clone()),
        &targets,
    )
    .unwrap();
    assert!(top.rank_by_fixture[&fixture(4)] < top.rank_by_fixture[&fixture(3)]);

    let front = evaluate_spatial_mapping(
        &mapping(ProjectionPreset::Front, ascending_up.clone()),
        &targets,
    )
    .unwrap();
    assert!(front.rank_by_fixture[&fixture(6)] < front.rank_by_fixture[&fixture(5)]);

    let back = evaluate_spatial_mapping(
        &mapping(ProjectionPreset::Back, ascending_x.clone()),
        &targets,
    )
    .unwrap();
    assert!(back.rank_by_fixture[&fixture(2)] < back.rank_by_fixture[&fixture(1)]);

    for preset in [ProjectionPreset::Left, ProjectionPreset::Right] {
        let side =
            evaluate_spatial_mapping(&mapping(preset, ascending_up.clone()), &targets).unwrap();
        assert!(side.rank_by_fixture[&fixture(6)] < side.rank_by_fixture[&fixture(5)]);
    }
}

#[test]
fn projected_position_preview_uses_ranking_plane_and_retains_missing_targets() {
    let projection = SpatialProjection::from_preset(
        ProjectionPreset::Top,
        Position3d {
            x: 10.0,
            y: 20.0,
            z: 5.0,
        },
    );
    let targets = [
        target(1, 12.0, 17.0, 9.0),
        SpatialTarget {
            fixture_id: fixture(2),
            position: None,
        },
        target(1, 99.0, 99.0, 99.0),
    ];

    assert_eq!(
        project_spatial_positions(&projection, &targets).unwrap(),
        [
            ProjectedSpatialPosition {
                fixture_id: fixture(1),
                u: Some(2.0),
                v: Some(-3.0),
            },
            ProjectedSpatialPosition {
                fixture_id: fixture(2),
                u: None,
                v: None,
            },
        ]
    );
}

#[test]
fn anchor_rotation_and_negative_zero_are_deterministic() {
    let mut projection = SpatialProjection::from_preset(
        ProjectionPreset::Top,
        Position3d {
            x: 10.0,
            y: 20.0,
            z: 5.0,
        },
    );
    projection.rotation_degrees = 90.0;
    projection.preset = None;
    let ranked = evaluate_spatial_mapping(
        &SpatialSelectionMapping {
            projection,
            shape: SpatialSelectionShape::Grid {
                angle_degrees: -0.0,
                direction: RankDirection::Ascending,
            },
        },
        &[target(1, 10.0, 19.0, 5.0), target(2, 10.0, 21.0, 5.0)],
    )
    .unwrap();
    assert_eq!(ranked.ordered_fixture_ids, [fixture(1), fixture(2)]);
}

#[test]
fn equal_spatial_keys_share_ranks_and_missing_positions_remain_individual() {
    let targets = [
        target(1, 0.0, 0.0, 0.0),
        target(2, 0.0, 2.0, 0.0),
        target(3, 2.0, 0.0, 0.0),
        SpatialTarget {
            fixture_id: fixture(4),
            position: None,
        },
        SpatialTarget {
            fixture_id: fixture(5),
            position: None,
        },
        target(1, 99.0, 99.0, 99.0),
    ];
    let ranked = evaluate_spatial_mapping(
        &mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Grid {
                angle_degrees: 0.0,
                direction: RankDirection::Ascending,
            },
        ),
        &targets,
    )
    .unwrap();

    assert_eq!(
        ranked.ordered_fixture_ids,
        [fixture(1), fixture(2), fixture(3), fixture(4), fixture(5)]
    );
    assert_eq!(ranked.rank_by_fixture[&fixture(1)], 0);
    assert_eq!(ranked.rank_by_fixture[&fixture(2)], 0);
    assert_eq!(ranked.rank_by_fixture[&fixture(3)], 1);
    assert_eq!(ranked.rank_by_fixture[&fixture(4)], 2);
    assert_eq!(ranked.rank_by_fixture[&fixture(5)], 3);
    assert_eq!(ranked.rank_count, 4);
    assert_eq!(ranked.warnings.len(), 2);
}

#[test]
fn dynamic_mapping_inherits_and_replaces_each_spatial_stage_independently() {
    let targets = [target(1, -2.0, 0.0, 3.0), target(2, 2.0, 0.0, -3.0)];
    let inherited = mapping(
        ProjectionPreset::Top,
        SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    );

    let inherited_result = evaluate_dynamic_spatial_mapping(
        Some(&inherited),
        &DynamicSpatialMappingOverride::default(),
        &targets,
        Some(99),
    )
    .unwrap();
    assert_eq!(
        inherited_result,
        evaluate_spatial_mapping(&inherited, &targets).unwrap(),
        "non-Random mappings ignore the loop index"
    );

    let front = SpatialProjection::from_preset(ProjectionPreset::Front, Position3d::default());
    let projection_override = DynamicSpatialMappingOverride {
        projection: OverrideStage::Replace(front.clone()),
        shape: OverrideStage::Inherit,
    };
    assert_eq!(
        evaluate_dynamic_spatial_mapping(Some(&inherited), &projection_override, &targets, None,)
            .unwrap(),
        evaluate_spatial_mapping(
            &SpatialSelectionMapping {
                projection: front,
                shape: inherited.shape.clone(),
            },
            &targets,
        )
        .unwrap()
    );

    let local_shape = DynamicSelectionShape::Radial {
        center_u: 0.0,
        center_v: 0.0,
        direction: RadialDirection::Inward,
    };
    let shape_override = DynamicSpatialMappingOverride {
        projection: OverrideStage::Inherit,
        shape: OverrideStage::Replace(local_shape),
    };
    assert_eq!(
        evaluate_dynamic_spatial_mapping(Some(&inherited), &shape_override, &targets, None,)
            .unwrap(),
        evaluate_spatial_mapping(
            &SpatialSelectionMapping {
                projection: inherited.projection.clone(),
                shape: SpatialSelectionShape::Radial {
                    center_u: 0.0,
                    center_v: 0.0,
                    direction: RadialDirection::Inward,
                },
            },
            &targets,
        )
        .unwrap()
    );
}

#[test]
fn dynamic_mapping_without_a_group_uses_independent_stage_fallbacks() {
    let targets = [
        target(3, 3.0, 0.0, 0.0),
        target(1, 1.0, 0.0, 0.0),
        target(3, 99.0, 99.0, 99.0),
        target(2, 2.0, 0.0, 0.0),
    ];
    let source_order = evaluate_dynamic_spatial_mapping(
        None,
        &DynamicSpatialMappingOverride::default(),
        &targets,
        Some(7),
    )
    .unwrap();
    assert_eq!(
        source_order.ordered_fixture_ids,
        [fixture(3), fixture(1), fixture(2)]
    );
    assert_eq!(source_order.rank_count, 3);
    assert!(source_order.warnings.is_empty());

    let projection_only = DynamicSpatialMappingOverride {
        projection: OverrideStage::Replace(SpatialProjection::from_preset(
            ProjectionPreset::Top,
            Position3d::default(),
        )),
        shape: OverrideStage::Inherit,
    };
    let projection_only_result =
        evaluate_dynamic_spatial_mapping(None, &projection_only, &targets, None).unwrap();
    assert_eq!(projection_only_result, source_order);

    let shape_only = DynamicSpatialMappingOverride {
        projection: OverrideStage::Inherit,
        shape: OverrideStage::Replace(DynamicSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        }),
    };
    let shape_only_result =
        evaluate_dynamic_spatial_mapping(None, &shape_only, &targets, None).unwrap();
    assert_eq!(
        shape_only_result,
        evaluate_spatial_mapping(
            &SpatialSelectionMapping {
                projection: SpatialProjection::from_preset(
                    ProjectionPreset::Top,
                    Position3d::default(),
                ),
                shape: SpatialSelectionShape::Grid {
                    angle_degrees: 0.0,
                    direction: RankDirection::Ascending,
                },
            },
            &targets,
        )
        .unwrap()
    );

    let complete = DynamicSpatialMappingOverride {
        projection: projection_only.projection,
        shape: shape_only.shape,
    };
    assert!(
        evaluate_dynamic_spatial_mapping(None, &complete, &targets, None).is_ok(),
        "both local spatial stages remain valid without a Group mapping"
    );
}

#[test]
fn dynamic_random_is_position_independent_repeatable_and_loop_perturbed() {
    let targets = [
        SpatialTarget {
            fixture_id: fixture(1),
            position: None,
        },
        target(2, f64::NAN, 0.0, 0.0),
        target(3, 30.0, 20.0, 10.0),
        target(4, -4.0, -3.0, -2.0),
        target(5, 5.0, 4.0, 3.0),
        target(2, 999.0, 999.0, 999.0),
    ];
    let random = DynamicSpatialMappingOverride {
        projection: OverrideStage::Inherit,
        shape: OverrideStage::Replace(DynamicSelectionShape::Random { seed: 0x5eed }),
    };
    let mut invalid_projection_random = random.clone();
    invalid_projection_random.projection = OverrideStage::Replace(SpatialProjection {
        anchor: Position3d {
            x: f64::NAN,
            y: f64::NAN,
            z: f64::NAN,
        },
        view_direction: Vector3::default(),
        rotation_degrees: f64::NAN,
        preset: None,
        kind: ProjectionKind::Planar,
    });

    let base = evaluate_dynamic_spatial_mapping(None, &random, &targets, None).unwrap();
    assert_eq!(
        base.ordered_fixture_ids,
        [fixture(1), fixture(3), fixture(4), fixture(5), fixture(2)]
    );
    assert_eq!(base.rank_count, 5);
    assert!(base.warnings.is_empty());
    assert_eq!(
        evaluate_dynamic_spatial_mapping(None, &random, &targets, None).unwrap(),
        base
    );
    assert_eq!(
        evaluate_dynamic_spatial_mapping(None, &invalid_projection_random, &targets, None,)
            .unwrap(),
        base,
        "Random ignores even an invalid local projection"
    );

    let loop_seven = evaluate_dynamic_spatial_mapping(None, &random, &targets, Some(7)).unwrap();
    assert_eq!(
        loop_seven.ordered_fixture_ids,
        [fixture(4), fixture(2), fixture(1), fixture(3), fixture(5)]
    );
    assert_ne!(loop_seven.ordered_fixture_ids, base.ordered_fixture_ids);
}

#[test]
fn dynamic_spatial_mapping_preserves_position_warnings() {
    let inherited = mapping(
        ProjectionPreset::Top,
        SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    );
    let missing = SpatialTarget {
        fixture_id: fixture(7),
        position: None,
    };

    let ranked = evaluate_dynamic_spatial_mapping(
        Some(&inherited),
        &DynamicSpatialMappingOverride::default(),
        &[target(6, 0.0, 0.0, 0.0), missing],
        None,
    )
    .unwrap();

    assert_eq!(
        ranked.warnings,
        [SpatialMappingWarning::MissingPosition {
            fixture_id: fixture(7)
        }]
    );
}

#[test]
fn radial_and_radar_directions_produce_literal_rank_order() {
    let targets = [
        target(1, 0.0, 0.0, 0.0),
        target(2, 1.0, 0.0, 0.0),
        target(3, 0.0, 1.0, 0.0),
        target(4, -1.0, 0.0, 0.0),
    ];
    let outward = evaluate_spatial_mapping(
        &mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Radial {
                center_u: 0.0,
                center_v: 0.0,
                direction: RadialDirection::Outward,
            },
        ),
        &targets,
    )
    .unwrap();
    assert_eq!(outward.rank_by_fixture[&fixture(1)], 0);
    assert_eq!(outward.rank_by_fixture[&fixture(2)], 1);
    assert_eq!(outward.rank_by_fixture[&fixture(3)], 1);

    let inward = evaluate_spatial_mapping(
        &mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Radial {
                center_u: 0.0,
                center_v: 0.0,
                direction: RadialDirection::Inward,
            },
        ),
        &targets,
    )
    .unwrap();
    assert!(inward.rank_by_fixture[&fixture(2)] < inward.rank_by_fixture[&fixture(1)]);

    let clockwise = evaluate_spatial_mapping(
        &mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Radar {
                center_u: 0.0,
                center_v: 0.0,
                start_angle_degrees: 0.0,
                sweep: RadarSweep::Clockwise,
            },
        ),
        &targets[1..],
    )
    .unwrap();
    let counter_clockwise = evaluate_spatial_mapping(
        &mapping(
            ProjectionPreset::Top,
            SpatialSelectionShape::Radar {
                center_u: 0.0,
                center_v: 0.0,
                start_angle_degrees: 0.0,
                sweep: RadarSweep::CounterClockwise,
            },
        ),
        &targets[1..],
    )
    .unwrap();
    assert_ne!(
        clockwise.ordered_fixture_ids,
        counter_clockwise.ordered_fixture_ids
    );
}

#[test]
fn invalid_vectors_and_non_finite_configuration_are_rejected() {
    let mut invalid = mapping(
        ProjectionPreset::Top,
        SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    );
    invalid.projection.view_direction = Vector3::default();
    assert_eq!(
        evaluate_spatial_mapping(&invalid, &[]),
        Err(SpatialMappingError::InvalidViewDirection)
    );
    invalid.projection.view_direction = ProjectionPreset::Top.view_direction();
    invalid.projection.anchor.x = f64::NAN;
    assert_eq!(
        evaluate_spatial_mapping(&invalid, &[]),
        Err(SpatialMappingError::NonFinite {
            field: "projection.anchor.x"
        })
    );
}

#[test]
fn decoding_clears_a_stale_preset_hint_and_defaults_dynamic_override_to_inherit() {
    let projection: SpatialProjection = serde_json::from_value(serde_json::json!({
        "anchor": {"x": 0.0, "y": 0.0, "z": 0.0},
        "view_direction": {"x": 0.0, "y": 1.0, "z": 0.0},
        "rotation_degrees": 15.0,
        "preset": "front"
    }))
    .unwrap();
    assert_eq!(projection.preset, None);

    let override_state: DynamicSpatialMappingOverride =
        serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(override_state, DynamicSpatialMappingOverride::default());
}
