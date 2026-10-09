use super::*;
use light_core::programming::{PositionIntent, TargetReference};

#[test]
fn saved_aim_targets_share_point_and_mount_semantics_and_skip_missing_relations() {
    let mut reference = point();
    reference.fixture_number = Some(5);
    reference.location.x = 4000;
    let mut target = mounted(reference.fixture_id, 2000);
    target.fixture_number = Some(6);
    // Saved Point defaults are centred; its translation/rotation must not distort a
    // mounted target's saved world position (2 m relative to the 4 m origin = -2 m). The reference retains the root UUID.
    for head in &mut reference.definition.heads {
        for parameter in &mut head.parameters {
            parameter.default = 0.5;
        }
    }
    let mut missing = target.clone();
    missing.fixture_id = FixtureId::new();
    missing.fixture_number = Some(7);
    missing.position_master = Some(uuid::Uuid::new_v4());
    let fixtures = vec![reference.clone(), target, missing];
    let targets = crate::saved_aim_targets(&fixtures, [5, 6, 7, 999]);
    assert_eq!(
        targets[&5],
        PositionIntent::target(
            TargetReference::Point {
                point_id: reference.fixture_id.0
            },
            [0.0; 3]
        )
    );
    assert_eq!(
        targets[&6],
        PositionIntent::target(
            TargetReference::Point {
                point_id: reference.fixture_id.0
            },
            [-2.0, 0.0, 0.0]
        )
    );
    assert!(!targets.contains_key(&7));
    assert!(!targets.contains_key(&999));
    let (engine, _) = engine(fixtures.clone());
    let frame = engine.observe_source_frame(&[]);
    assert_eq!(
        crate::aim_target_from_geometry(&fixtures, frame.points(), frame.mounts(), 6).unwrap(),
        Some(targets[&6].clone())
    );
}
