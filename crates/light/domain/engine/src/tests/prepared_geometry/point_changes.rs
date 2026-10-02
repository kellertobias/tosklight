//! Lookup/deltas use immutable captured Point poses, including Points that mount nothing.
use super::*;
use crate::PreparedFrameGeometry;
use uuid::Uuid;

const AXES: [&str; 6] = [
    X,
    Y,
    "point.position.z",
    "point.rotation.x",
    "point.rotation.y",
    "point.rotation.z",
];
fn id(value: u128) -> FixtureId {
    FixtureId(Uuid::from_u128(value))
}
fn point_fixture(id: FixtureId) -> PatchedFixture {
    let channels = AXES.map(|axis| (axis, false, false, false, false, false));
    let (mut fixture, _) = schema_v2_fixture(&channels);
    fixture.fixture_id = id;
    fixture.fixture_number = None;
    fixture.universe = None;
    fixture.address = None;
    fixture
}
fn rig(ids: &[FixtureId]) -> Engine {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    for target in ids {
        for axis in AXES {
            registry.set(
                session,
                *target,
                AttributeKey(axis.into()),
                AttributeValue::Normalized(0.5),
            );
        }
    }
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: ids
                .iter()
                .copied()
                .map(point_fixture)
                .collect::<Vec<_>>()
                .into(),
            ..Default::default()
        })
        .unwrap();
    engine
}
fn geometry(engine: &Engine) -> PreparedFrameGeometry {
    let capture = engine.prepare_output_frame(Default::default());
    let mut frame = engine.prepare_static_family_frame(&capture, &[]);
    engine
        .observe_static_family_geometry(&capture, &mut frame)
        .unwrap()
}

#[test]
fn point_lookup_uses_sorted_captured_rows_and_does_no_projection_work() {
    let unsorted = [id(97), id(3), id(41)];
    let engine = rig(&unsorted);
    let capture = engine.prepare_output_frame(Default::default());
    let mut frame = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut frame)
        .unwrap();
    let resolutions = capture.generation.point_projection().resolutions();
    assert_eq!(
        geometry
            .points()
            .iter()
            .map(|point| point.fixture_id)
            .collect::<Vec<_>>(),
        [id(3), id(41), id(97)]
    );
    for target in unsorted {
        let found = geometry.point(target).unwrap();
        assert_eq!(found.fixture_id, target);
        assert_eq!(found.offset_metres, [0.; 3]);
        assert_eq!(found.rotation_degrees, [0.; 3]);
        assert!(std::ptr::eq(
            found,
            geometry
                .points()
                .iter()
                .find(|point| point.fixture_id == target)
                .unwrap()
        ));
    }
    for absent in [id(1), id(20), id(100)] {
        assert!(geometry.point(absent).is_none());
    }
    let mut changes = Vec::with_capacity(16);
    changes.push(id(999));
    let allocation = changes.as_ptr();
    geometry.changed_points_into(&geometry, &mut changes);
    assert!(changes.is_empty());
    assert_eq!(changes.as_ptr(), allocation);
    assert_eq!(
        capture.generation.point_projection().resolutions(),
        resolutions
    );
}

#[test]
fn point_translation_and_rotation_delta_retains_prior_lane_geometry_and_reuses_output() {
    let engine = rig(&[id(97), id(3), id(41)]);
    let capture = engine.prepare_output_frame(Default::default());
    let continuity = engine.capture_output_continuity().0;
    let mut before_frame = engine.prepare_static_family_frame(&capture, &[]);
    let before = engine
        .observe_static_family_geometry(&capture, &mut before_frame)
        .unwrap();
    let retained = before.clone();
    let old_poses = before.points().to_vec();
    let mut moved_frame = engine.prepare_static_family_frame(
        &capture,
        &[
            sample(id(97), X, 0.6),
            sample(id(3), "point.rotation.z", 0.75),
        ],
    );
    let moved = engine
        .observe_static_family_geometry(&capture, &mut moved_frame)
        .unwrap();
    assert_eq!(before.generation(), moved.generation());
    assert_eq!(before.sampled_at(), moved.sampled_at());
    let mut changes = Vec::with_capacity(16);
    changes.push(id(999));
    let allocation = changes.as_ptr();
    moved.changed_points_into(&before, &mut changes);
    assert_eq!(
        changes,
        [id(3), id(97)],
        "Point-only aims invalidate even when these Points mount no fixtures"
    );
    assert_eq!(changes.as_ptr(), allocation);
    near(
        f64::from(moved.point(id(97)).unwrap().offset_metres[0]),
        20.,
    );
    near(
        f64::from(moved.point(id(3)).unwrap().rotation_degrees[2]),
        90.,
    );
    assert_eq!(before.points(), old_poses);
    assert_eq!(retained.points(), old_poses);
    assert!(Arc::ptr_eq(&before.points, &retained.points));
    before.changed_points_into(&moved, &mut changes);
    assert_eq!(changes, [id(3), id(97)]);
    let moved_again = engine
        .observe_static_family_geometry(&capture, &mut moved_frame)
        .unwrap();
    moved_again.changed_points_into(&moved, &mut changes);
    assert!(changes.is_empty());
    let foreign = engine.prepare_output_frame(Default::default());
    assert!(
        engine
            .observe_static_family_geometry(&foreign, &mut before_frame)
            .is_err()
    );
    assert_eq!(before.points(), old_poses);
    assert_eq!(
        engine.capture_output_continuity().0,
        continuity,
        "lookup/delta/observation never accept output continuity"
    );
}

#[test]
fn point_generation_change_reports_sorted_union_including_additions_removals_and_equal_poses() {
    let engine = rig(&[id(97), id(3), id(41)]);
    let before = geometry(&engine);
    let mut snapshot = engine.snapshot().as_ref().clone();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures.retain(|fixture| fixture.fixture_id != id(3));
    fixtures.push(point_fixture(id(20)));
    fixtures
        .iter_mut()
        .find(|fixture| fixture.fixture_id == id(97))
        .unwrap()
        .location
        .x = 4_000;
    snapshot.fixtures = fixtures.into();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    let after = geometry(&engine);
    assert_ne!(before.generation(), after.generation());
    assert_eq!(
        before.point(id(41)),
        after.point(id(41)),
        "unchanged pose still invalidates with generation"
    );
    assert!(before.point(id(20)).is_none());
    assert!(after.point(id(3)).is_none());
    assert_eq!(after.point(id(97)).unwrap().origin_metres[0], 4.);
    let mut changes = Vec::with_capacity(16);
    after.changed_points_into(&before, &mut changes);
    assert_eq!(changes, [id(3), id(20), id(41), id(97)]);
    before.changed_points_into(&after, &mut changes);
    assert_eq!(changes, [id(3), id(20), id(41), id(97)]);
    let mut snapshot = engine.snapshot().as_ref().clone();
    snapshot.fixtures = Vec::new().into();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    let empty = geometry(&engine);
    assert!(empty.point(id(41)).is_none());
    empty.changed_points_into(&after, &mut changes);
    assert_eq!(changes, [id(20), id(41), id(97)]);
    after.changed_points_into(&empty, &mut changes);
    assert_eq!(changes, [id(20), id(41), id(97)]);
    empty.changed_points_into(&empty, &mut changes);
    assert!(changes.is_empty());
}
