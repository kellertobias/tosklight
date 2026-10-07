//! A 3D Point that a tracking system holds, and everything that must not move it.
//!
//! The attribute here is `point.position.x` because that is what a 3D Point actually carries, but
//! nothing in the engine is fussy about the name: what is being tested is that an override
//! outranks each source in turn, and that dropping the binding hands the attribute straight back.

use super::*;

fn tracked_point() -> (PatchedFixture, FixtureId) {
    schema_v2_fixture(&[("point.position.x", false, false)])
}

fn nonshared_point() -> (PatchedFixture, FixtureId, FixtureId) {
    let (mut fixture, root) = schema_v2_fixture(&[
        ("point.position.x", false, false),
        ("point.position.y", false, false),
        ("point.position.z", false, false),
        ("point.rotation.x", false, false),
        ("point.rotation.y", false, false),
        ("point.rotation.z", false, false),
    ]);
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode_id = profile.modes[0].id;
    let head_id = profile.modes[0].heads[0].id;
    profile.modes[0].heads[0].master_shared = false;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    let child = FixtureId::new();
    fixture.logical_heads = vec![PatchedHead {
        profile_head_id: Some(head_id),
        head_index: 0,
        fixture_id: child,
    }];
    fixture.universe = None;
    fixture.address = None;
    (fixture, root, child)
}

fn held(fixture_id: FixtureId, value: f32) -> crate::TrackedOverride {
    crate::TrackedOverride::new(
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(value),
    )
}

fn point_at(engine: &Engine, fixture_id: FixtureId) -> f32 {
    normalized(&engine.resolved_values(), fixture_id, "point.position.x")
}

fn world_input(
    fixture_id: FixtureId,
    position_metres: [f32; 3],
    sequence: u64,
) -> Arc<crate::TrackedInputFrame> {
    Arc::new(crate::TrackedInputFrame {
        configuration_generation: 4,
        point_generation: 2,
        source_generation: 3,
        accepted_sequence: sequence,
        sampled_at_millis: 120,
        points: vec![crate::TrackedPointInput {
            binding_id: uuid::Uuid::from_u128(1),
            fixture_id,
            position_metres,
            sample: crate::TrackedSampleIdentity {
                source_id: "tracker.example:56565".into(),
                source_generation: 3,
                source_epoch: 1,
                sequence,
                sender_timestamp_micros: 5_000,
                accepted_at_millis: 101,
            },
            position_received_at_millis: 100,
        }]
        .into(),
        ..Default::default()
    })
}

fn captured_point(engine: &Engine, frame: &crate::PreparedOutputFrame) -> crate::ResolvedPointPose {
    let mut continuity = frame.continuity.clone();
    let mut resolved = engine.resolve_prepared_attributes(frame, &[], &mut continuity, false);
    let values = resolved.named_values();
    let points = frame.generation.point_projection().resolve(&values);
    assert!(!values.materialised_by_name());
    points[0]
}

fn assert_world_point(point: crate::ResolvedPointPose, expected: [f32; 3]) {
    for axis in 0..3 {
        assert!(
            (point.origin_metres[axis] + point.offset_metres[axis] - expected[axis]).abs() < 0.0001
        );
    }
}

#[test]
fn captured_world_tracking_uses_each_frames_patch_origin_without_a_target_jump() {
    let (mut fixture, root, child) = nonshared_point();
    fixture.location = Default::default();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture.clone()].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let held = world_input(root, [10.0, 20.0, 30.0], 7);
    engine.set_tracking_frame(Arc::clone(&held));
    let before_patch = engine.prepare_output_frame(RenderOptions::default());

    fixture.location.x = 5_000;
    fixture.location.y = -4_000;
    fixture.location.z = 1_000;
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    // No new tracking tick occurs. The same world input is interpreted with the new origin.
    let after_patch = engine.prepare_output_frame(RenderOptions::default());
    let previous = captured_point(&engine, &before_patch);
    let current = captured_point(&engine, &after_patch);
    assert_eq!(previous.origin_metres, [0.0; 3]);
    assert_eq!(current.origin_metres, [5.0, -4.0, 1.0]);
    assert_world_point(previous, [10.0, 20.0, 30.0]);
    assert_world_point(current, [10.0, 20.0, 30.0]);
    assert!(Arc::ptr_eq(before_patch.tracking(), &held));
    assert!(Arc::ptr_eq(after_patch.tracking(), &held));

    engine.set_tracking_frame(world_input(root, [40.0, 50.0, 60.0], 8));
    assert_world_point(captured_point(&engine, &after_patch), [10.0, 20.0, 30.0]);
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_world_point(rendered.points[0], [40.0, 50.0, 60.0]);
    // Root binding reaches the logical head's actual storage, not a synthetic root attribute.
    assert!(
        (rendered
            .resolved_values
            .value(child, &AttributeKey("point.position.x".into()))
            .and_then(AttributeValue::normalized)
            .unwrap()
            - 0.675)
            .abs()
            < 0.00001
    );
    assert_eq!(after_patch.tracking().points[0].sample.sequence, 7);
    assert_eq!(
        after_patch.tracking().points[0].position_age_millis(120),
        20
    );
}

#[test]
fn tracked_world_range_is_applied_to_the_captured_origin_and_deleted_points_are_skipped() {
    let (mut fixture, root, _) = nonshared_point();
    fixture.location = Default::default();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture.clone()].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine.set_tracking_frame(world_input(root, [140.0, 0.0, 0.0], 7));
    assert_world_point(
        engine.render(RenderOptions::default()).unwrap().points[0],
        [100.0, 0.0, 0.0],
    );
    fixture.location.x = 50_000;
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    assert_world_point(
        engine.render(RenderOptions::default()).unwrap().points[0],
        [140.0, 0.0, 0.0],
    );
    assert_eq!(
        engine.tracking_frame().points[0].position_metres,
        [140.0, 0.0, 0.0]
    );

    engine
        .replace_snapshot(EngineSnapshot {
            revision: 3,
            ..Default::default()
        })
        .unwrap();
    assert!(
        engine
            .render(RenderOptions::default())
            .unwrap()
            .points
            .is_empty()
    );
    assert!(engine.tracked_overrides().is_empty());
    engine.set_tracked_overrides([held(root, 0.25)]);
    assert!(engine.tracking_frame().points.is_empty());
    assert_eq!(engine.tracked_overrides(), vec![held(root, 0.25)]);
    engine.clear_tracked_overrides();
    assert_eq!(
        *engine.tracking_frame(),
        crate::TrackedInputFrame::default()
    );
}

#[test]
fn world_tracking_never_encodes_nonfinite_positions_and_age_cannot_underflow() {
    let root = FixtureId::new();
    let input = world_input(root, [f32::MAX, -f32::MAX, 0.0], 1);
    assert_eq!(
        input.points[0].normalized_position([0.0; 3]),
        Some([1.0, 0.0, 0.5])
    );
    assert_eq!(
        input.points[0].normalized_position([f32::NAN, 0.0, 0.0]),
        None
    );
    assert_eq!(input.points[0].position_age_millis(50), 0);
    let invalid = world_input(root, [0.0, f32::INFINITY, 0.0], 2);
    assert_eq!(invalid.points[0].normalized_position([0.0; 3]), None);
}

#[test]
fn a_bound_point_ignores_the_programmer() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = tracked_point();
    programmers.set(
        session,
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.2),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    assert_eq!(point_at(&engine, fixture_id), 0.2);

    engine.set_tracked_overrides([held(fixture_id, 0.75)]);
    assert_eq!(point_at(&engine, fixture_id), 0.75);
}

#[test]
fn a_bound_point_ignores_a_running_cue() {
    let (fixture, fixture_id) = tracked_point();
    let cue_list = test_cue_list(
        "Positions",
        vec![CueChange::set(
            fixture_id,
            AttributeKey("point.position.x".into()),
            AttributeValue::Normalized(0.1),
        )],
    );
    let mut playback = test_playback(1, cue_list.id);
    playback.auto_off = false;
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.6)]);

    // The cue goes live against a point that is already bound: the marker keeps it.
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    assert_eq!(point_at(&engine, fixture_id), 0.6);
}

#[test]
fn unbinding_hands_the_point_back() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = tracked_point();
    programmers.set(
        session,
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.2),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.75)]);
    assert_eq!(point_at(&engine, fixture_id), 0.75);

    // Unbinding is the operator's way of taking the point back, and it is immediate: the
    // programmer value that was underneath all along is what the point reads again.
    engine.clear_tracked_overrides();
    assert_eq!(point_at(&engine, fixture_id), 0.2);
    assert!(engine.tracked_overrides().is_empty());
}

#[test]
fn a_source_that_goes_quiet_holds_where_it_last_was() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = tracked_point();
    programmers.set(
        session,
        fixture_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.2),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.8)]);

    // No further frames arrive. Nothing tells the engine so, which is the point: the last
    // position stays held rather than snapping back to the programmer mid-show.
    for _ in 0..3 {
        assert_eq!(point_at(&engine, fixture_id), 0.8);
    }
}

#[test]
fn only_the_bound_point_is_held() {
    let (bound, bound_id) = tracked_point();
    let (mut loose, loose_id) = tracked_point();
    // A second point of the same kind, patched somewhere else in the rig.
    loose.fixture_number = Some(2);
    loose.address = Some(10);
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    for fixture_id in [bound_id, loose_id] {
        programmers.set(
            session,
            fixture_id,
            AttributeKey("point.position.x".into()),
            AttributeValue::Normalized(0.3),
        );
    }
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![bound, loose].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(bound_id, 0.9)]);

    assert_eq!(point_at(&engine, bound_id), 0.9);
    assert_eq!(point_at(&engine, loose_id), 0.3);
}

#[test]
fn the_render_path_holds_the_point_as_well_as_the_read_path() {
    let (fixture, fixture_id) = tracked_point();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.5)]);

    let rendered = engine.render(RenderOptions::default()).unwrap();
    let value = rendered.resolved_values[&(fixture_id, AttributeKey("point.position.x".into()))]
        .normalized()
        .unwrap();
    assert_eq!(value, 0.5);
    assert_eq!(rendered.points.len(), 1);
    assert_eq!(rendered.points[0].fixture_id, fixture_id);
    assert_eq!(rendered.points[0].offset_metres, [0.0, 0.0, 0.0]);
}

#[test]
fn point_pose_reads_dense_values_without_materialising_named_map() {
    let (fixture, fixture_id) = tracked_point();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([held(fixture_id, 0.75)]);

    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.points[0].offset_metres, [50.0, 0.0, 0.0]);
    assert_eq!(rendered.points[0].rotation_degrees, [0.0, 0.0, 0.0]);
    assert!(!rendered.resolved_values.materialised_by_name());
}

#[test]
fn nonshared_point_reads_its_logical_head_but_reports_the_root_identity() {
    let (fixture, root, child) = nonshared_point();
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_tracked_overrides([
        crate::TrackedOverride::new(
            root,
            AttributeKey("point.position.x".into()),
            AttributeValue::Normalized(0.55),
        ),
        crate::TrackedOverride::new(
            root,
            AttributeKey("point.rotation.y".into()),
            AttributeValue::Normalized(0.75),
        ),
    ]);

    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_eq!(rendered.points.len(), 1);
    assert_eq!(rendered.points[0].fixture_id, root);
    assert!((rendered.points[0].offset_metres[0] - 10.0).abs() < 0.0001);
    assert_eq!(rendered.points[0].rotation_degrees, [0.0, 90.0, 0.0]);
    assert_eq!(
        rendered
            .resolved_values
            .value(child, &AttributeKey("point.position.x".into()))
            .and_then(AttributeValue::normalized),
        Some(0.55)
    );
    assert!(!rendered.resolved_values.materialised_by_name());

    let preload = engine
        .profile_visualization_projection(&engine.resolved_values(), RenderOptions::default())
        .unwrap();
    assert_eq!(preload.points[0].fixture_id, root);
    assert!((preload.points[0].offset_metres[0] - 10.0).abs() < 0.0001);
    assert_eq!(preload.points[0].rotation_degrees, [0.0, 90.0, 0.0]);
}
