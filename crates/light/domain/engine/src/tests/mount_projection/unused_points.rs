//! TL-614: an aim Point without mounted dependents changes captured geometry independently of
//! the mount cache. An unchanged mount-frame Arc proves only that mounted bodies did not move;
//! it never certifies that a Point a Target may depend on stayed still.

use super::*;

/// Desk-axis pose of one Point, compared by value rather than by allocation identity.
type Pose = ([f32; 3], [f32; 3], [f32; 3]);

fn pose_of(geometry: &PreparedFrameGeometry, id: FixtureId) -> Pose {
    let point = geometry.point(id).expect("captured Point pose");
    (
        point.origin_metres,
        point.offset_metres,
        point.rotation_degrees,
    )
}

fn near32(actual: [f32; 3], expected: [f32; 3]) {
    near(actual.map(f64::from), expected.map(f64::from));
}

fn set(engine: &Engine, session: SessionId, point: FixtureId, axis: usize, value: f32) {
    engine.programmers.set(
        session,
        point,
        AttributeKey(AXES[axis].into()),
        AttributeValue::Normalized(value),
    );
}

/// Capture, observe and finally render one static lane. The prepared geometry and the rendered
/// frame are both returned so every step can check that they are the same retained values.
fn frame(engine: &Engine, sampled: &[ContributionBatch]) -> (PreparedFrameGeometry, RenderResult) {
    let capture = engine.prepare_output_frame(Default::default());
    let mut lane = engine.prepare_static_family_frame(&capture, sampled);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut lane)
        .unwrap();
    let rendered = engine.render_static_family_frame(&capture, lane).unwrap();
    assert_eq!(rendered.points.as_slice(), geometry.points());
    assert!(Arc::ptr_eq(&rendered.points, &geometry.points));
    assert!(Arc::ptr_eq(&rendered.mounts, &geometry.mounts));
    (geometry, rendered)
}

/// The tempting but insufficient certificate: "the mount frame is the same allocation".
fn mount_identity_certifies_unchanged(
    previous: &PreparedFrameGeometry,
    current: &PreparedFrameGeometry,
) -> bool {
    Arc::ptr_eq(&previous.mounts, &current.mounts)
}

fn changed(previous: &PreparedFrameGeometry, current: &PreparedFrameGeometry) -> Vec<FixtureId> {
    let mut out = vec![FixtureId(uuid::Uuid::nil())];
    current.changed_points_into(previous, &mut out);
    out
}

struct Rig {
    engine: Engine,
    session: SessionId,
    aim: FixtureId,
    mount_point: FixtureId,
    root: uuid::Uuid,
    copy: uuid::Uuid,
    unrelated: uuid::Uuid,
}

/// An unmounted aim Point, a mounting Point carrying a root and a multipatch copy, and a
/// separate Point carrying an unrelated placement.
fn rig() -> Rig {
    let mut aim = point();
    aim.location = FixtureLocation {
        x: 5_000,
        y: -1_000,
        z: 2_000,
    };
    let mut mount_point = point();
    mount_point.location = FixtureLocation {
        x: 1_000,
        y: 2_000,
        z: 3_000,
    };
    let other = point();
    let mut fixture = mounted(mount_point.fixture_id, 3_000);
    fixture.location.y = 2_000;
    fixture.location.z = 3_000;
    fixture.rotation.x = 90.0;
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
    let unrelated = mounted(other.fixture_id, 9_000);
    let rig_ids = (
        aim.fixture_id,
        mount_point.fixture_id,
        fixture.fixture_id.0,
        fixture.multipatch[0].id,
        unrelated.fixture_id.0,
    );
    let (engine, session) = engine(vec![aim, mount_point, other, fixture, unrelated]);
    Rig {
        engine,
        session,
        aim: rig_ids.0,
        mount_point: rig_ids.1,
        root: rig_ids.2,
        copy: rig_ids.3,
        unrelated: rig_ids.4,
    }
}

#[test]
fn unmounted_aim_point_moves_captured_geometry_while_mount_cache_stays_shared() {
    let rig = rig();
    let engine = &rig.engine;
    engine.render(Default::default()).unwrap();
    let (base, base_render) = frame(engine, &[]);
    assert_eq!(work(engine), (0, 0));
    let base_aim = pose_of(&base, rig.aim);
    near32(base_aim.0, [5.0, -1.0, 2.0]);
    near32(base_aim.1, [0.0; 3]);
    near32(base_aim.2, [0.0; 3]);
    let base_root = transform(&base.mounts, rig.root);
    let base_copy = transform(&base.mounts, rig.copy);
    let base_unrelated = *base.mounts.mount(rig.unrelated).unwrap();
    let base_points: Vec<ResolvedPointPose> = base.points().to_vec();

    // Translation of the unmounted aim Point through the authoritative Programmer.
    set(engine, rig.session, rig.aim, 0, 0.51);
    let (translated, _) = frame(engine, &[]);
    assert_eq!(work(engine), (0, 0));
    let moved = pose_of(&translated, rig.aim);
    near32(moved.1, [2.0, 0.0, 0.0]);
    near32(moved.2, [0.0; 3]);
    assert_ne!(moved, base_aim);
    assert!(Arc::ptr_eq(&base.mounts, &translated.mounts));
    // Mount identity alone would wrongly certify the Target-relevant Point as unchanged.
    assert!(mount_identity_certifies_unchanged(&base, &translated));
    assert_eq!(changed(&base, &translated), vec![rig.aim]);

    // Rotation of the same Point, again without any mounted dependent.
    set(engine, rig.session, rig.aim, 4, 0.25);
    let (rotated, _) = frame(engine, &[]);
    assert_eq!(work(engine), (0, 0));
    let turned = pose_of(&rotated, rig.aim);
    near32(turned.1, [2.0, 0.0, 0.0]);
    near32(turned.2, [0.0, -90.0, 0.0]);
    assert!(Arc::ptr_eq(&base.mounts, &rotated.mounts));
    assert!(mount_identity_certifies_unchanged(&translated, &rotated));
    assert_eq!(changed(&translated, &rotated), vec![rig.aim]);

    // Moving the mounting Point invalidates exactly its root and copy.
    set(engine, rig.session, rig.mount_point, 2, 0.505);
    set(engine, rig.session, rig.mount_point, 5, 0.75);
    let (carried, carried_render) = frame(engine, &[]);
    assert_eq!(work(engine), (1, 2));
    assert!(!mount_identity_certifies_unchanged(&rotated, &carried));
    assert_eq!(changed(&rotated, &carried), vec![rig.mount_point]);
    assert_eq!(pose_of(&carried, rig.aim), turned);
    near(
        transform(&carried.mounts, rig.root).point([0.0; 3]),
        [1.0, 4.0, 4.0],
    );
    near(
        transform(&carried.mounts, rig.copy).point([0.0; 3]),
        [-2.0, 2.0, 4.0],
    );
    let turn = RigidTransform::euler_xyz([0.0, 0.0, 90.0]).unwrap();
    near(
        transform(&carried.mounts, rig.root).direction([0.0, 1.0, 0.0]),
        turn.compose(RigidTransform::euler_xyz([90.0, 0.0, 0.0]).unwrap())
            .direction([0.0, 1.0, 0.0]),
    );
    assert_eq!(
        carried.mounts.mount(rig.unrelated),
        Some(&base_unrelated),
        "an unrelated placement is copied, never recomputed"
    );

    // Retained geometry from earlier captures is immutable.
    assert_eq!(base.points(), base_points.as_slice());
    assert_eq!(base_render.points.as_slice(), base_points.as_slice());
    assert_eq!(pose_of(&base, rig.aim), base_aim);
    assert_eq!(pose_of(&translated, rig.aim), moved);
    assert_eq!(transform(&base.mounts, rig.root), base_root);
    assert_eq!(transform(&base.mounts, rig.copy), base_copy);
    assert_eq!(transform(&rotated.mounts, rig.root), base_root);
    assert_eq!(carried_render.points.as_slice(), carried.points());

    // A further aim-only edit after the mount moved again shares the newest mount frame.
    set(engine, rig.session, rig.aim, 1, 0.49);
    let (aim_again, _) = frame(engine, &[]);
    assert_eq!(work(engine), (0, 0));
    assert!(Arc::ptr_eq(&carried.mounts, &aim_again.mounts));
    assert_eq!(changed(&carried, &aim_again), vec![rig.aim]);
    near32(pose_of(&aim_again, rig.aim).1, [2.0, -2.0, 0.0]);
}

fn sample(point: FixtureId, axis: usize, value: f32) -> ContributionSample {
    ContributionSample::independent(TimedValue {
        fixture_id: point,
        attribute: AttributeKey(AXES[axis].into()),
        value: AttributeValue::Normalized(value),
        priority: 10_000,
        changed_at: Utc::now(),
        programmer_order: 0,
        merge_mode: light_core::MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })
}

#[test]
fn sampled_aim_contribution_changes_capture_but_not_the_mount_frame() {
    let rig = rig();
    let engine = &rig.engine;
    engine.render(Default::default()).unwrap();
    let (base, _) = frame(engine, &[]);
    let batch = ContributionBatch::new([sample(rig.aim, 0, 0.49), sample(rig.aim, 5, 0.75)]);
    let (sampled, rendered) = frame(engine, &[batch]);
    assert_eq!(work(engine), (0, 0));
    let aim = pose_of(&sampled, rig.aim);
    near32(aim.1, [-2.0, 0.0, 0.0]);
    near32(aim.2, [0.0, 0.0, 90.0]);
    assert_eq!(rendered.points.as_slice(), sampled.points());
    assert!(Arc::ptr_eq(&base.mounts, &sampled.mounts));
    assert!(mount_identity_certifies_unchanged(&base, &sampled));
    assert_eq!(changed(&base, &sampled), vec![rig.aim]);

    // A sampled mounting-Point contribution, by contrast, forks the mount frame for its
    // dependents only, and the aim Point returns to its Programmer pose without the sample.
    let batch = ContributionBatch::new([sample(rig.mount_point, 2, 0.505)]);
    let (carried, _) = frame(engine, &[batch]);
    assert_eq!(work(engine), (1, 2));
    assert!(!Arc::ptr_eq(&sampled.mounts, &carried.mounts));
    let mut expected = vec![rig.aim, rig.mount_point];
    expected.sort_by_key(|id| id.0);
    assert_eq!(changed(&sampled, &carried), expected);
    near(
        transform(&carried.mounts, rig.root).point([0.0; 3]),
        [3.0, 2.0, 4.0],
    );
    near(
        transform(&carried.mounts, rig.copy).point([0.0; 3]),
        [1.0, 5.0, 4.0],
    );
    assert_eq!(
        carried.mounts.mount(rig.unrelated),
        base.mounts.mount(rig.unrelated)
    );
    assert_eq!(pose_of(&carried, rig.aim), pose_of(&base, rig.aim));
    near32(pose_of(&sampled, rig.aim).1, [-2.0, 0.0, 0.0]);
}

#[test]
fn preview_of_an_aim_edit_matches_prepared_geometry_and_leaves_live_mounts_alone() {
    let rig = rig();
    let engine = &rig.engine;
    let live = engine.render(Default::default()).unwrap();
    set(engine, rig.session, rig.aim, 3, 0.75);
    let capture = engine.prepare_output_frame(Default::default());
    let mut lane = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut lane)
        .unwrap();
    near32(pose_of(&geometry, rig.aim).2, [90.0, 0.0, 0.0]);
    let preview = engine.preview_static_family_frame(&capture, lane).unwrap();
    assert!(Arc::ptr_eq(&preview.points, &geometry.points));
    assert!(Arc::ptr_eq(&preview.mounts, &geometry.mounts));
    assert!(Arc::ptr_eq(&live.mounts, &preview.mounts));
    let after = engine.render(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&live.mounts, &after.mounts));
    assert_eq!(after.points.as_slice(), geometry.points());
    assert_ne!(after.points.as_slice(), live.points.as_slice());
}
