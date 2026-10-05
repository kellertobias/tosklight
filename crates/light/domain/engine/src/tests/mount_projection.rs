use super::*;
use light_core::spatial::RigidTransform;
use light_fixture::{FixtureLocation, FixtureVector};

mod unused_points;

const AXES: [&str; 6] = [
    "point.position.x",
    "point.position.y",
    "point.position.z",
    "point.rotation.x",
    "point.rotation.y",
    "point.rotation.z",
];

fn point() -> PatchedFixture {
    let channels: Vec<_> = AXES.iter().map(|name| (*name, false, false)).collect();
    let (mut fixture, _) = schema_v2_fixture(&channels);
    fixture.fixture_number = None;
    fixture.universe = None;
    fixture.address = None;
    fixture
}

fn mounted(point: FixtureId, x: i32) -> PatchedFixture {
    let (mut fixture, _) = schema_v2_fixture(&[("pan", false, false)]);
    fixture.fixture_number = None;
    fixture.universe = None;
    fixture.address = None;
    fixture.position_master = Some(point.0);
    fixture.location.x = x;
    fixture
}

fn engine(fixtures: Vec<PatchedFixture>) -> (Engine, SessionId) {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    for point in fixtures.iter().filter(|f| {
        f.definition.heads.iter().any(|head| {
            head.parameters
                .iter()
                .any(|p| p.attribute.0.as_ref() == AXES[0])
        })
    }) {
        for axis in AXES {
            registry.set(
                session,
                point.fixture_id,
                AttributeKey(axis.into()),
                AttributeValue::Normalized(0.5),
            );
        }
    }
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            ..Default::default()
        })
        .unwrap();
    (engine, session)
}

fn near(actual: [f64; 3], expected: [f64; 3]) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() < 0.0001,
            "{actual:?} != {expected:?}"
        );
    }
}

fn transform(frame: &FixtureMountFrame, instance: uuid::Uuid) -> RigidTransform {
    frame.mount(instance).unwrap().world_from_fixture.unwrap()
}

fn work(engine: &Engine) -> (usize, usize) {
    let state = engine.output_continuity.lock();
    (
        state.mounts.recomputed_points,
        state.mounts.recomputed_mounts,
    )
}

#[test]
fn moving_one_point_recomputes_only_its_root_and_copy_and_keeps_old_output_immutable() {
    let mut first = point();
    first.location = FixtureLocation {
        x: 1_000,
        y: 2_000,
        z: 3_000,
    };
    let first_id = first.fixture_id;
    let other = point();
    let mut fixture = mounted(first_id, 3_000);
    fixture.location.y = 2_000;
    fixture.location.z = 3_000;
    fixture.rotation.x = 90.0;
    fixture.position_calibration = Some(light_fixture::InstalledPositionCalibration {
        pan_zero_degrees: 42.0,
        tilt_zero_degrees: -13.0,
        ..Default::default()
    });
    fixture.multipatch.push(MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        location: FixtureLocation {
            x: 1_000,
            y: 5_000,
            z: 3_000,
        },
        rotation: FixtureVector {
            x: 0.0,
            y: 90.0,
            z: 0.0,
        },
        ..Default::default()
    });
    let root = fixture.fixture_id.0;
    let copy = fixture.multipatch[0].id;
    let unrelated = mounted(other.fixture_id, 9_000);
    let unrelated_id = unrelated.fixture_id.0;
    let (engine, session) = engine(vec![first, other, fixture, unrelated]);
    let before = engine.render(Default::default()).unwrap();
    assert_eq!(work(&engine), (2, 3));
    assert_eq!(before.mounts.mounts().len(), 5);
    let original_root = transform(&before.mounts, root);
    engine.programmers.set(
        session,
        first_id,
        AttributeKey(AXES[2].into()),
        AttributeValue::Normalized(0.505),
    );
    engine.programmers.set(
        session,
        first_id,
        AttributeKey(AXES[5].into()),
        AttributeValue::Normalized(0.75),
    );
    let moved = engine.render(Default::default()).unwrap();
    assert_eq!(work(&engine), (1, 2));
    near(
        transform(&moved.mounts, root).point([0.0; 3]),
        [1.0, 4.0, 4.0],
    );
    near(
        transform(&moved.mounts, copy).point([0.0; 3]),
        [-2.0, 2.0, 4.0],
    );
    let turn = RigidTransform::euler_xyz([0.0, 0.0, 90.0]).unwrap();
    near(
        transform(&moved.mounts, root).direction([0.0, 1.0, 0.0]),
        turn.compose(RigidTransform::euler_xyz([90.0, 0.0, 0.0]).unwrap())
            .direction([0.0, 1.0, 0.0]),
    );
    assert_eq!(
        moved.mounts.mount(unrelated_id),
        before.mounts.mount(unrelated_id)
    );
    assert_eq!(transform(&before.mounts, root), original_root);
    assert!(!moved.resolved_values.materialised_by_name());
    let unchanged = engine.render(Default::default()).unwrap();
    assert_eq!(work(&engine), (0, 0));
    assert!(Arc::ptr_eq(&moved.mounts, &unchanged.mounts));
}

#[test]
fn patch_calibration_rebind_and_deleted_point_invalidate_only_new_generations() {
    let point = point();
    let point_id = point.fixture_id;
    let fixture = mounted(point_id, 2_000);
    let id = fixture.fixture_id.0;
    let (engine, session) = engine(vec![point.clone(), fixture.clone()]);
    engine.programmers.set(
        session,
        point_id,
        AttributeKey(AXES[0].into()),
        AttributeValue::Normalized(0.55),
    );
    let first = engine.render(Default::default()).unwrap();
    let old = engine.prepare_output_frame(Default::default());
    let old_generation = engine.generation.load_full();
    let mut revision_only = (*engine.snapshot()).clone();
    revision_only.revision += 1;
    engine.replace_snapshot(revision_only).unwrap();
    assert!(std::ptr::eq(
        old_generation.mount_projection(),
        engine.generation.load().mount_projection()
    ));
    let unchanged = engine.render(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&first.mounts, &unchanged.mounts));
    let mut calibrated = fixture;
    calibrated.position_calibration = Some(light_fixture::InstalledPositionCalibration {
        pan_zero_degrees: 30.0,
        ..Default::default()
    });
    calibrated.location.x = 4_000;
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![point, calibrated.clone()].into(),
            revision: 3,
            ..Default::default()
        })
        .unwrap();
    assert!(!std::ptr::eq(
        old_generation.mount_projection(),
        engine.generation.load().mount_projection()
    ));
    let updated = engine.render(Default::default()).unwrap();
    near(
        transform(&updated.mounts, id).point([0.0; 3]),
        [14.0, 0.0, 0.0],
    );
    let old_lane = engine.prepare_static_family_frame(&old, &[]);
    let retained = engine.preview_static_family_frame(&old, old_lane).unwrap();
    near(
        transform(&retained.mounts, id).point([0.0; 3]),
        [12.0, 0.0, 0.0],
    );
    assert!(Arc::ptr_eq(&retained.mounts, &first.mounts));
    calibrated.position_master = Some(uuid::Uuid::new_v4());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![calibrated].into(),
            revision: 4,
            ..Default::default()
        })
        .unwrap();
    let fallback = engine.render(Default::default()).unwrap();
    assert_eq!(fallback.mounts.mount(id).unwrap().position_master, None);
    near(
        transform(&fallback.mounts, id).point([0.0; 3]),
        [4.0, 0.0, 0.0],
    );
    near(
        transform(&updated.mounts, id).point([0.0; 3]),
        [14.0, 0.0, 0.0],
    );
}

fn x_sample(point: FixtureId, value: f32) -> ContributionBatch {
    ContributionBatch::new([ContributionSample::independent(TimedValue {
        fixture_id: point,
        attribute: AttributeKey(AXES[0].into()),
        value: AttributeValue::Normalized(value),
        priority: 10_000,
        changed_at: Utc::now(),
        programmer_order: 0,
        merge_mode: light_core::MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })])
}

#[test]
fn final_sampled_point_drives_mounts_while_preview_and_failed_render_leave_live_cache_alone() {
    let point = point();
    let point_id = point.fixture_id;
    let fixture = mounted(point_id, 2_000);
    let id = fixture.fixture_id.0;
    let (engine, _) = engine(vec![point, fixture]);
    let live = engine.render(Default::default()).unwrap();
    let capture = engine.prepare_output_frame(Default::default());
    let lane = engine.prepare_static_family_frame(&capture, &[x_sample(point_id, 0.55)]);
    let preview = engine.preview_static_family_frame(&capture, lane).unwrap();
    near(
        transform(&preview.mounts, id).point([0.0; 3]),
        [12.0, 0.0, 0.0],
    );
    near(
        preview.points[0].offset_metres.map(f64::from),
        [10.0, 0.0, 0.0],
    );
    let still_live = engine.render(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&live.mounts, &still_live.mounts));
    let mut broken = engine.prepare_output_frame(Default::default());
    broken.generation = Arc::new(RuntimeGeneration::new(
        (*broken.snapshot()).clone(),
        broken.generation.playback_arc(),
        Default::default(),
        Default::default(),
        Default::default(),
    ));
    let lane = engine.prepare_static_family_frame(&broken, &[x_sample(point_id, 0.6)]);
    assert!(engine.render_static_family_frame(&broken, lane).is_err());
    let still_live = engine.render(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&live.mounts, &still_live.mounts));
    let capture = engine.prepare_output_frame(Default::default());
    let lane = engine.prepare_static_family_frame(&capture, &[x_sample(point_id, 0.55)]);
    let final_output = engine.render_static_family_frame(&capture, lane).unwrap();
    near(
        transform(&final_output.mounts, id).point([0.0; 3]),
        [12.0, 0.0, 0.0],
    );
    assert_eq!(work(&engine), (1, 1));
}

#[test]
fn preload_keeps_an_independent_reusable_mount_workspace() {
    use light_programmer::{PreloadProgrammerValueMutation, PreloadProgrammerValueTiming};
    let point = point();
    let point_id = point.fixture_id;
    let fixture = mounted(point_id, 2_000);
    let id = fixture.fixture_id.0;
    let (engine, session) = engine(vec![point, fixture]);
    let live = engine.render(Default::default()).unwrap();
    engine.programmers.arm_preload(session, true);
    engine.programmers.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: point_id,
            attribute: AttributeKey(AXES[0].into()),
            value: AttributeValue::Normalized(0.55),
            timing: PreloadProgrammerValueTiming::default(),
        }],
    );
    let capture = engine.prepare_output_frame(Default::default());
    let pending = engine.prepare_preload_frame(&capture, None);
    let mut state = PreloadFrameState::default();
    let first = engine
        .render_prepared_preload(&pending, &[], &[], &mut state)
        .unwrap();
    near(
        transform(first.source.mounts(), id).point([0.0; 3]),
        [12.0, 0.0, 0.0],
    );
    let second = engine
        .render_prepared_preload(&pending, &[], &[], &mut state)
        .unwrap();
    assert!(Arc::ptr_eq(&first.source.mounts, &second.source.mounts));
    let observed = engine.observe_prepared_frame(&capture, &[]);
    near(
        transform(observed.mounts(), id).point([0.0; 3]),
        [2.0, 0.0, 0.0],
    );
    let live_again = engine.render(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&live.mounts, &live_again.mounts));
}

#[test]
fn invalid_saved_rotation_is_unavailable_without_breaking_output() {
    let point = point();
    let mut fixture = mounted(point.fixture_id, 2_000);
    fixture.rotation.x = f32::NAN;
    let id = fixture.fixture_id.0;
    let (engine, _) = engine(vec![point, fixture]);
    let output = engine.render(Default::default()).unwrap();
    assert!(
        output
            .mounts
            .mount(id)
            .unwrap()
            .world_from_fixture
            .is_none()
    );
}

#[test]
fn tracking_configuration_changes_use_captured_world_pose_through_logical_point_owner() {
    let mut point = point();
    point.location.x = 1_000;
    let point_id = point.fixture_id;
    let mut profile = point
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = &mut profile.modes[0];
    mode.heads[0].master_shared = false;
    let head_id = mode.heads[0].id;
    let mode_id = mode.id;
    let child = FixtureId::new();
    point.definition = profile.resolved_definition(mode_id).unwrap();
    point.logical_heads = vec![PatchedHead {
        profile_head_id: Some(head_id),
        head_index: 0,
        fixture_id: child,
    }];
    let fixture = mounted(point_id, 2_000);
    let id = fixture.fixture_id.0;
    let (engine, session) = engine(vec![point, fixture]);
    for axis in AXES {
        engine.programmers.set(
            session,
            child,
            AttributeKey(axis.into()),
            AttributeValue::Normalized(0.5),
        );
    }
    let input = |configuration_generation, x| {
        Arc::new(TrackedInputFrame {
            configuration_generation,
            points: vec![TrackedPointInput {
                binding_id: uuid::Uuid::from_u128(1),
                fixture_id: point_id,
                position_metres: [x, 0.0, 0.0],
                sample: TrackedSampleIdentity {
                    source_id: "test".into(),
                    source_generation: 1,
                    source_epoch: 1,
                    sequence: 1,
                    sender_timestamp_micros: 1,
                    accepted_at_millis: 1,
                },
                position_received_at_millis: 1,
            }]
            .into(),
            ..Default::default()
        })
    };
    engine.set_tracking_frame(input(1, 10.0));
    let first = engine.render(Default::default()).unwrap();
    let old = engine.prepare_output_frame(Default::default());
    let generation = engine.generation.load_full();
    engine.set_tracking_frame(input(2, 20.0));
    let next = engine.render(Default::default()).unwrap();
    assert!(std::ptr::eq(
        generation.mount_projection(),
        engine.generation.load().mount_projection()
    ));
    assert_eq!(work(&engine), (1, 1));
    near(
        transform(&first.mounts, id).point([0.0; 3]),
        [11.0, 0.0, 0.0],
    );
    near(
        transform(&next.mounts, id).point([0.0; 3]),
        [21.0, 0.0, 0.0],
    );
    let retained = engine.observe_prepared_frame(&old, &[]);
    near(
        transform(retained.mounts(), id).point([0.0; 3]),
        [11.0, 0.0, 0.0],
    );
    assert!(!retained.values().materialised_by_name());
    engine.set_tracking_frame(input(3, 20.0));
    let same_pose = engine.render(Default::default()).unwrap();
    assert_eq!(work(&engine), (0, 0));
    assert!(Arc::ptr_eq(&next.mounts, &same_pose.mounts));
}
