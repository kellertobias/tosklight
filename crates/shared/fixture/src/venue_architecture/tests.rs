use super::*;
use crate::SceneryOptions;
use serde_json::json;

fn opening(id: u128, offset: u32) -> WallOpening {
    WallOpening {
        id: Uuid::from_u128(id),
        offset_millimetres: offset,
        sill_millimetres: 800,
        width_millimetres: 1000,
        height_millimetres: 1000,
        opening: WallOpeningKind::Window {
            shape: VenueWindowShape::Rectangular,
        },
    }
}

fn wall(openings: Vec<WallOpening>) -> VenueArchitecture {
    VenueArchitecture::Wall {
        openings,
        ceiling_height_millimetres: Some(3200),
    }
}

fn slab(points: &[[i32; 2]]) -> VenueArchitecture {
    VenueArchitecture::Slab {
        purpose: VenueSlabPurpose::Balcony,
        outline_millimetres: points
            .iter()
            .map(|p| VenuePlanPoint { x: p[0], y: p[1] })
            .collect(),
        elevation_millimetres: 3000,
        thickness_millimetres: 200,
    }
}

#[test]
fn existing_options_remain_empty_and_byte_equivalent_in_json() {
    let old: SceneryOptions = serde_json::from_value(json!({})).unwrap();
    assert!(old.is_empty());
    assert_eq!(serde_json::to_string(&old).unwrap(), "{}");
    let coloured: SceneryOptions =
        serde_json::from_value(json!({"colour_srgb":"#102030"})).unwrap();
    assert!(coloured.architecture.is_none());
    assert_eq!(
        serde_json::to_value(coloured).unwrap(),
        json!({"colour_srgb":"#102030"})
    );
}

#[test]
fn wall_openings_retain_identity_shape_and_relationship_through_options() {
    let mut door = opening(2, 2200);
    door.sill_millimetres = 0;
    door.height_millimetres = 2100;
    door.opening = WallOpeningKind::Door {
        leaves: VenueDoorLeaves::Double,
        swing: VenueDoorSwing::Outward,
        hinge: VenueDoorHinge::Left,
    };
    let options = SceneryOptions {
        architecture: Some(wall(vec![opening(1, 100), door])),
        ..Default::default()
    };
    assert!(!options.is_empty());
    options.validate().unwrap();
    options
        .architecture
        .as_ref()
        .unwrap()
        .validate_placement_millimetres([4000., 3200., 180.])
        .unwrap();
    let value = serde_json::to_value(&options).unwrap();
    assert_eq!(value["architecture"]["kind"], "wall");
    assert_eq!(
        value["architecture"]["openings"][1]["opening"]["swing"],
        "outward"
    );
    let decoded: SceneryOptions = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, options);
}

#[test]
fn openings_require_unique_stable_ids_positive_dimensions_and_valid_shapes() {
    for bad in [
        vec![opening(0, 0)],
        vec![opening(1, 0), opening(1, 2000)],
        vec![WallOpening {
            width_millimetres: 0,
            ..opening(1, 0)
        }],
        vec![WallOpening {
            height_millimetres: 0,
            ..opening(1, 0)
        }],
        vec![WallOpening {
            width_millimetres: 1200,
            opening: WallOpeningKind::Window {
                shape: VenueWindowShape::Round,
            },
            ..opening(1, 0)
        }],
        vec![WallOpening {
            width_millimetres: 2100,
            opening: WallOpeningKind::Window {
                shape: VenueWindowShape::Arched,
            },
            ..opening(1, 0)
        }],
        vec![WallOpening {
            opening: WallOpeningKind::Door {
                leaves: VenueDoorLeaves::Single,
                swing: VenueDoorSwing::Inward,
                hinge: VenueDoorHinge::Right,
            },
            ..opening(1, 0)
        }],
    ] {
        assert!(wall(bad).validate().is_err());
    }
    wall(vec![WallOpening {
        opening: WallOpeningKind::Window {
            shape: VenueWindowShape::Round,
        },
        ..opening(1, 0)
    }])
    .validate()
    .unwrap();
    wall(vec![WallOpening {
        width_millimetres: 2000,
        opening: WallOpeningKind::Window {
            shape: VenueWindowShape::Arched,
        },
        ..opening(1, 0)
    }])
    .validate()
    .unwrap();
}

#[test]
fn opening_bounds_use_real_wall_size_and_do_not_overflow() {
    assert!(
        wall(vec![opening(1, 3500)])
            .validate_placement_millimetres([4000., 3200., 180.])
            .is_err()
    );
    assert!(
        wall(vec![opening(1, 0)])
            .validate_placement_millimetres([4000., 1700., 180.])
            .is_err()
    );
    assert!(
        wall(vec![opening(1, u32::MAX)])
            .validate_placement_millimetres([4000., 3200., 180.])
            .is_err()
    );
    assert!(
        wall(vec![opening(1, 0), opening(2, 500)])
            .validate()
            .is_err()
    );
    wall(vec![opening(1, 0), opening(2, 1000)])
        .validate()
        .unwrap();
    let upper = WallOpening {
        sill_millimetres: 1800,
        ..opening(2, 0)
    };
    wall(vec![opening(1, 0), upper]).validate().unwrap();
}

#[test]
fn slab_accepts_both_windings_concavity_and_stacking() {
    let points = [
        [0, 0],
        [4000, 0],
        [4000, 1000],
        [1000, 1000],
        [1000, 3000],
        [0, 3000],
    ];
    slab(&points).validate().unwrap();
    let mut reversed = points;
    reversed.reverse();
    slab(&reversed).validate().unwrap();
    let lower = slab(&points);
    let mut upper = lower.clone();
    if let VenueArchitecture::Slab {
        elevation_millimetres,
        ..
    } = &mut upper
    {
        *elevation_millimetres = 6000;
    }
    assert_ne!(lower, upper);
    assert_eq!(
        serde_json::from_value::<VenueArchitecture>(serde_json::to_value(&upper).unwrap()).unwrap(),
        upper
    );
}

#[test]
fn slab_rejects_empty_degenerate_touching_and_crossed_polygons() {
    for points in [
        vec![],
        vec![[0, 0], [1000, 0]],
        vec![[0, 0], [1000, 0], [2000, 0]],
        vec![[0, 0], [1000, 0], [1000, 0], [0, 1000]],
        vec![[0, 0], [1000, 1000], [0, 1000], [1000, 0]],
        vec![[0, 0], [3000, 0], [1000, 0], [1000, 1000], [0, 1000]],
        vec![[0, 0], [2000, 0], [2000, 2000], [1000, 0], [0, 2000]],
        vec![[0, 0], [1000, 0], [1000, 1000], [0, 1000], [0, 0]],
    ] {
        assert!(slab(&points).validate().is_err(), "{points:?}");
    }
    let mut zero = slab(&[[0, 0], [1000, 0], [0, 1000]]);
    if let VenueArchitecture::Slab {
        thickness_millimetres,
        ..
    } = &mut zero
    {
        *thickness_millimetres = 0;
    }
    assert!(zero.validate().is_err());
    slab(&[
        [i32::MIN, i32::MIN],
        [i32::MAX, i32::MIN],
        [i32::MAX, i32::MAX],
        [i32::MIN, i32::MAX],
    ])
    .validate()
    .unwrap();
}

#[test]
fn placement_dimensions_and_equal_footprints_are_validated() {
    let round = VenueArchitecture::Pillar {
        shape: VenueFootprint::Round,
    };
    for dimensions in [
        [0., 1., 1.],
        [1., -1., 1.],
        [f64::NAN, 1., 1.],
        [1., f64::INFINITY, 1.],
        [1., 1., 2.],
    ] {
        assert!(round.validate_placement_millimetres(dimensions).is_err());
    }
    round
        .validate_placement_millimetres([500., 3000., 500.])
        .unwrap();
    VenueArchitecture::Pillar {
        shape: VenueFootprint::Rectangular,
    }
    .validate_placement_millimetres([500., 3000., 800.])
    .unwrap();
    VenueArchitecture::Table {
        shape: VenueTableShape::Round,
    }
    .validate_placement_millimetres([1200., 800., 1200.])
    .unwrap();
    for style in [
        VenueChairStyle::BlackMetal,
        VenueChairStyle::White,
        VenueChairStyle::BarStool,
    ] {
        let chair = VenueArchitecture::Chair { style };
        assert_eq!(
            serde_json::from_value::<VenueArchitecture>(serde_json::to_value(&chair).unwrap())
                .unwrap(),
            chair
        );
    }
    VenueArchitecture::Bar
        .validate_placement_millimetres([2400., 1200., 800.])
        .unwrap();
}

#[test]
fn scenery_options_cannot_bypass_architecture_validation() {
    let options = SceneryOptions {
        architecture: Some(wall(vec![opening(0, 0)])),
        ..Default::default()
    };
    assert!(options.validate().is_err());
    assert!(
        VenueArchitecture::Wall {
            openings: vec![],
            ceiling_height_millimetres: Some(0)
        }
        .validate()
        .is_err()
    );
}

#[test]
fn stairs_steps_and_placement_dimensions_are_validated_and_roundtrip() {
    for steps in [0, 257, u32::MAX] {
        let stairs = VenueArchitecture::Stairs { steps };
        assert!(stairs.validate().unwrap_err().contains("1 to 256 steps"));
        assert!(
            SceneryOptions {
                architecture: Some(stairs),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    for steps in [1, 16, 256] {
        let stairs = VenueArchitecture::Stairs { steps };
        stairs
            .validate_placement_millimetres([1200.0, 3000.0, 4500.0])
            .unwrap();
        let encoded = serde_json::to_value(&stairs).unwrap();
        assert_eq!(encoded, json!({"kind":"stairs","steps":steps}));
        assert_eq!(
            serde_json::from_value::<VenueArchitecture>(encoded).unwrap(),
            stairs
        );
        for dimensions in [
            [0.0, 3000.0, 4500.0],
            [1200.0, -1.0, 4500.0],
            [1200.0, 3000.0, 0.0],
            [f64::NAN, 3000.0, 4500.0],
        ] {
            assert!(stairs.validate_placement_millimetres(dimensions).is_err());
        }
    }
}

#[test]
fn floor_slab_purpose_roundtrips_with_its_dimensions_and_elevation() {
    let floor = VenueArchitecture::Slab {
        purpose: VenueSlabPurpose::Floor,
        outline_millimetres: vec![
            VenuePlanPoint { x: 0, y: 0 },
            VenuePlanPoint { x: 4000, y: 0 },
            VenuePlanPoint { x: 0, y: 3000 },
        ],
        elevation_millimetres: 0,
        thickness_millimetres: 150,
    };
    floor
        .validate_placement_millimetres([4000.0, 150.0, 3000.0])
        .unwrap();
    let encoded = serde_json::to_value(&floor).unwrap();
    assert_eq!(encoded["purpose"], "floor");
    assert_eq!(
        serde_json::from_value::<VenueArchitecture>(encoded).unwrap(),
        floor
    );
}
