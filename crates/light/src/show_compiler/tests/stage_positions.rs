use super::super::compile_show_candidate;
use super::support::document_with_objects;
use light_core::FixtureId;
use light_dynamics::SpatialPosition;
use serde_json::json;
use uuid::Uuid;

#[test]
fn dynamic_stage_positions_retain_xyz_for_fixture_and_logical_head_identities() {
    let fixture_id = Uuid::new_v4();
    let logical_head_id = Uuid::new_v4();
    let legacy_2d_id = Uuid::new_v4();
    let objects = vec![(
        "stage_layout",
        "main",
        json!({
            "version": 2,
            "positions3d": {
                fixture_id.to_string(): {"x": 1.25, "y": 2.5, "z": 3.75},
                logical_head_id.to_string(): {"x": -4.0, "y": -5.0, "z": -6.0}
            },
            "positions": {
                legacy_2d_id.to_string(): {"x": 7.0, "y": 8.0}
            }
        }),
    )];
    let (_, document) = document_with_objects(&objects);

    let snapshot =
        compile_show_candidate(document.candidate(&document.transaction()).unwrap()).unwrap();

    assert_eq!(
        snapshot.dynamic_stage_positions.get(&FixtureId(fixture_id)),
        Some(&SpatialPosition {
            x: 1.25,
            y: 2.5,
            z: 3.75,
        })
    );
    assert_eq!(
        snapshot
            .dynamic_stage_positions
            .get(&FixtureId(logical_head_id)),
        Some(&SpatialPosition {
            x: -4.0,
            y: -5.0,
            z: -6.0,
        })
    );
    assert_eq!(
        snapshot
            .dynamic_stage_positions
            .get(&FixtureId(legacy_2d_id)),
        Some(&SpatialPosition {
            x: 7.0,
            y: 0.0,
            z: 8.0,
        })
    );
}
