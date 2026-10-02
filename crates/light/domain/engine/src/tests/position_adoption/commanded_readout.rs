//! Selected-owner readout of actual commanded joints from ordinary engine forward output.
//! Synthetic profiles prove identity/calibration/precision guards, not measured lamp accuracy
//! or acceptance of arbitrary observational frames. No bulk hot-render performance is claimed.
use super::*;

#[test]
fn commanded_readout_keeps_each_emitter_and_divergent_mounted_copy_separate() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    redefine(&mut fixture, |profile| {
        let mut emitter = profile.geometry.emitters[0].clone();
        emitter.id = Uuid::new_v4();
        emitter.name = "Second owned lens".into();
        let head_id = profile.modes[0].emitter_heads[0].head_id;
        profile.modes[0].emitter_heads.push(EmitterHeadBinding {
            emitter_id: emitter.id,
            head_id,
        });
        profile.geometry.emitters.push(emitter);
    });
    let emitters = fixture
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap()
        .geometry
        .emitters
        .iter()
        .map(|emitter| emitter.id)
        .collect::<Vec<_>>();
    let copy = Uuid::new_v4();
    fixture.multipatch.push(MultiPatchInstance {
        id: copy,
        universe: Some(1),
        address: Some(10),
        location: light_fixture::FixtureLocation {
            x: 2000,
            y: -1000,
            z: 500,
        },
        rotation: light_fixture::FixtureVector {
            x: 0.,
            y: 0.,
            z: 45.,
        },
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: 360.,
            tilt_zero_degrees: 20.,
            ..Default::default()
        }),
        ..Default::default()
    });
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, root, "tilt", 0.);
    let rendered = engine.render(Default::default()).unwrap();
    let before = engine.snapshot();
    let readouts = engine
        .position_commanded_readout_from_physical(rendered.generation, &rendered.physical, root)
        .unwrap();
    assert_eq!(readouts.len(), 4);
    assert_eq!(
        readouts
            .iter()
            .map(|readout| (readout.destination, readout.emitter_id))
            .collect::<Vec<_>>(),
        vec![
            (root, emitters[0]),
            (root, emitters[1]),
            (FixtureId(copy), emitters[0]),
            (FixtureId(copy), emitters[1])
        ]
    );
    for readout in &readouts {
        assert!(readout.pan_degrees.is_finite() && readout.tilt_degrees.is_finite());
        let expected = if readout.destination == root {
            [720., -720.]
        } else {
            [1080., -700.]
        };
        assert_eq!([readout.pan_degrees, readout.tilt_degrees], expected);
    }
    assert_ne!(
        rendered.physical.instances[0].lenses[0].world,
        rendered.physical.instances[1].lenses[0].world,
        "actual different mount/calibration output"
    );
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        None
    );
    assert_eq!(
        engine.position_commanded_readout_from_physical(
            rendered.generation,
            &rendered.physical,
            FixtureId(copy)
        ),
        None,
        "a physical copy is not a programming owner"
    );
    assert!(
        Arc::ptr_eq(&before, &engine.snapshot()),
        "readout does not replace or accept engine state"
    );
    assert_eq!(
        readouts,
        engine
            .position_commanded_readout_from_physical(rendered.generation, &rendered.physical, root)
            .unwrap(),
        "same supplied frame, no resampling"
    );
}

#[test]
fn commanded_readout_agrees_with_common_adoption_without_reapplying_inversion_or_calibration() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    let calibration = InstalledPositionCalibration {
        pan_zero_degrees: 15.,
        tilt_zero_degrees: -10.,
        ..Default::default()
    };
    fixture.position_calibration = Some(calibration.clone());
    let copy = Uuid::new_v4();
    fixture.multipatch.push(MultiPatchInstance {
        id: copy,
        universe: Some(1),
        address: Some(10),
        invert_pan: true,
        position_calibration: Some(calibration),
        ..Default::default()
    });
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, root, "tilt", 0.);
    let rendered = engine.render(Default::default()).unwrap();
    assert_ne!(
        rendered.physical.instances[0].native_raw,
        rendered.physical.instances[1].native_raw
    );
    let readouts = engine
        .position_commanded_readout_from_physical(rendered.generation, &rendered.physical, root)
        .unwrap();
    assert_eq!(readouts.len(), 2);
    let common = engine
        .position_angles_from_physical(rendered.generation, &rendered.physical, root)
        .unwrap();
    for readout in readouts {
        assert_eq!([readout.pan_degrees, readout.tilt_degrees], [735., -730.]);
        assert_eq!(
            [readout.pan_degrees as f32, readout.tilt_degrees as f32],
            [common.pan_degrees, common.tilt_degrees]
        );
    }
}

#[test]
fn common_adoption_still_refuses_distinct_f64_commands_that_round_to_the_same_f32() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    fixture.multipatch.push(MultiPatchInstance {
        id: Uuid::new_v4(),
        universe: Some(1),
        address: Some(10),
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: 0.000001,
            ..Default::default()
        }),
        ..Default::default()
    });
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    let rendered = engine.render(Default::default()).unwrap();
    let readouts = engine
        .position_commanded_readout_from_physical(rendered.generation, &rendered.physical, root)
        .unwrap();
    assert_ne!(readouts[0].pan_degrees, readouts[1].pan_degrees);
    assert_eq!(
        readouts[0].pan_degrees as f32,
        readouts[1].pan_degrees as f32
    );
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        None,
        "copy equality is checked before the authoring f32 conversion"
    );
    let bulk = engine
        .position_commanded_readouts_from_physical(rendered.generation, &rendered.physical, &[root])
        .unwrap();
    assert_eq!(
        bulk[0].common_angles(),
        None,
        "bulk common conversion shares strict f64 equality"
    );
}

#[test]
fn commanded_readout_rejects_stale_foreign_incomplete_and_unsupported_frames() {
    let fixture = mover();
    let root = fixture.fixture_id;
    let (engine, _, _) = engine(fixture.clone());
    let rendered = engine.render(Default::default()).unwrap();
    assert!(
        engine
            .position_commanded_readout_from_physical(rendered.generation, &rendered.physical, root)
            .is_some()
    );
    assert_eq!(
        engine.position_commanded_readout_from_physical(
            rendered.generation + 1,
            &rendered.physical,
            root
        ),
        None
    );
    let (foreign, _, _) = engine_fixture(fixture);
    let foreign_frame = foreign.render(Default::default()).unwrap();
    assert_eq!(
        engine.position_commanded_readout_from_physical(
            rendered.generation,
            &foreign_frame.physical,
            root
        ),
        None
    );
    let incomplete = engine
        .generation
        .load_full()
        .physical_projection()
        .take_frame();
    assert_eq!(
        engine.position_commanded_readout_from_physical(rendered.generation, &incomplete, root),
        None
    );
    assert_eq!(
        engine.position_commanded_readout_from_physical(
            rendered.generation,
            &rendered.physical,
            FixtureId::new()
        ),
        None
    );
    engine
        .replace_snapshot(engine.snapshot().as_ref().clone())
        .unwrap();
    let fresh = engine.render(Default::default()).unwrap();
    assert_eq!(
        engine.position_commanded_readout_from_physical(
            rendered.generation,
            &rendered.physical,
            root
        ),
        None
    );
    assert_eq!(
        engine.position_commanded_readout_from_physical(fresh.generation, &rendered.physical, root),
        None
    );
    assert!(
        engine
            .position_commanded_readout_from_physical(fresh.generation, &fresh.physical, root)
            .is_some()
    );

    let mut velocity = mover();
    let owner = velocity.fixture_id;
    redefine(&mut velocity, |profile| {
        let function = &mut profile.modes[0].channels[0].functions[0];
        function.angular_motion.as_mut().unwrap().kind = AngularMotionKind::AngularVelocity;
        if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut function.behavior {
            *unit = Some("deg/s".into());
        }
    });
    let (velocity_engine, _, _) = engine_fixture(velocity);
    let velocity_frame = velocity_engine.render(Default::default()).unwrap();
    assert!(velocity_frame.physical.instances[0].complete);
    assert_eq!(
        velocity_engine.position_commanded_readout_from_physical(
            velocity_frame.generation,
            &velocity_frame.physical,
            owner
        ),
        None
    );
}

#[test]
fn bulk_readout_preserves_owner_order_and_isolates_duplicate_or_unavailable_rows() {
    let first = mover();
    let root = first.fixture_id;
    let mut second = mover();
    second.fixture_number = Some(2);
    second.address = Some(20);
    let other = second.fixture_id;
    let (engine, programmers, session) = engine(first);
    let snapshot = engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures.push(second);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, other, "pan", 0.);
    let unavailable = FixtureId::new();
    let requested = [other, unavailable, root, other];
    let mut rendered = engine.render(Default::default()).unwrap();
    let rows = engine
        .position_commanded_readouts_from_physical(
            rendered.generation,
            &rendered.physical,
            &requested,
        )
        .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.owner).collect::<Vec<_>>(),
        requested
    );
    assert!(rows[1].commands.is_none());
    assert_eq!(rows[1].common_angles(), None);
    assert_eq!(
        rows[0].commands, rows[3].commands,
        "repeated input owner retains its own row"
    );
    for index in [0, 2, 3] {
        assert_eq!(
            rows[index].commands,
            engine.position_commanded_readout_from_physical(
                rendered.generation,
                &rendered.physical,
                requested[index]
            )
        );
        assert_eq!(
            rows[index].common_angles(),
            engine.position_angles_from_physical(
                rendered.generation,
                &rendered.physical,
                requested[index]
            )
        );
    }
    assert_eq!(rows[0].commands.as_ref().unwrap()[0].pan_degrees, -720.);
    assert_eq!(rows[2].commands.as_ref().unwrap()[0].pan_degrees, 720.);
    assert_eq!(
        engine.position_commanded_readouts_from_physical(
            rendered.generation,
            &rendered.physical,
            &[]
        ),
        Some(Vec::new())
    );
    assert_eq!(
        engine.position_commanded_readouts_from_physical(
            rendered.generation + 1,
            &rendered.physical,
            &requested
        ),
        None
    );

    // Inject a duplicate actual row from another real render of this same generation, so the
    // frame identity remains valid. Duplicate key handling must not be first/last-wins, and
    // does not invalidate a disjoint owner's otherwise complete output.
    let mut same_generation = engine.render(Default::default()).unwrap();
    let index = same_generation
        .physical
        .instances
        .iter()
        .position(|row| row.fixture_id == root)
        .unwrap();
    let duplicate = Arc::get_mut(&mut same_generation.physical)
        .expect("ordinary render returns this new frame with one Arc owner")
        .instances
        .remove(index);
    Arc::get_mut(&mut rendered.physical)
        .expect("test has not cloned the supplied rendered frame")
        .instances
        .push(duplicate);
    let rejected = engine
        .position_commanded_readouts_from_physical(
            rendered.generation,
            &rendered.physical,
            &requested,
        )
        .unwrap();
    assert_eq!(rejected[0].commands, rows[0].commands);
    assert!(rejected[1].commands.is_none());
    assert!(rejected[2].commands.is_none());
    assert_eq!(rejected[3].commands, rows[3].commands);
    assert_eq!(
        engine.position_commanded_readout_from_physical(
            rendered.generation,
            &rendered.physical,
            root
        ),
        None
    );
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        None
    );

    let (foreign, _, _) = engine_fixture(mover());
    let foreign_frame = foreign.render(Default::default()).unwrap();
    assert_eq!(
        engine.position_commanded_readouts_from_physical(
            rendered.generation,
            &foreign_frame.physical,
            &requested
        ),
        None
    );
}
