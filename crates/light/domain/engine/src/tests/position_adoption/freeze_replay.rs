//! Accepted native Position holds travel through the ordinary final renderer.
use super::*;

fn replace_fixture(engine: &Engine, fixture: &PatchedFixture) {
    let mut snapshot = engine.snapshot().as_ref().clone();
    Arc::make_mut(&mut snapshot.fixtures)[0] = fixture.clone();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
}

fn capture_hold(engine: &Engine, fixture: &mut PatchedFixture, full: bool) {
    let accepted = engine.render(Default::default()).unwrap();
    let native = engine
        .position_freeze_from_physical(accepted.generation, &accepted.physical, fixture.fixture_id)
        .unwrap();
    fixture.freeze.targets.insert(
        fixture.fixture_id,
        FrozenFixtureTarget {
            full,
            families: if full {
                Vec::new()
            } else {
                vec![FreezeFamily::Position]
            },
            position_native: Some(native),
            ..Default::default()
        },
    );
    replace_fixture(engine, fixture);
}

fn words(rendered: &crate::RenderResult, instance: Uuid) -> Vec<u32> {
    let output = rendered
        .physical
        .instances
        .iter()
        .find(|i| i.instance_id == instance)
        .unwrap();
    assert!(output.complete);
    output.native_raw.to_vec()
}

fn assert_dmx(rendered: &crate::RenderResult, address: usize, width: usize, words: &[u32]) {
    let expected: Vec<_> = words
        .iter()
        .flat_map(|raw| raw.to_be_bytes()[4 - width..].to_vec())
        .collect();
    assert_eq!(
        &rendered.universes[&1][address - 1..address - 1 + expected.len()],
        expected.as_slice()
    );
}

#[test]
fn native_freeze_replays_exact_root_copy_words_and_resumes_after_removal() {
    for width in [2, 4] {
        let mut fixture = mover();
        let root = fixture.fixture_id;
        if width == 4 {
            redefine(&mut fixture, |profile| {
                let mode = &mut profile.modes[0];
                mode.splits[0].footprint = 8;
                for (index, channel) in mode.channels.iter_mut().enumerate() {
                    channel.resolution = ChannelResolution::U32;
                    channel.secondary_slots =
                        (2 + 4 * index as u16..=4 + 4 * index as u16).collect();
                    channel.functions[0].dmx_to = u32::MAX;
                }
            });
        }
        let copy = Uuid::new_v4();
        fixture.multipatch.push(MultiPatchInstance {
            id: copy,
            universe: Some(1),
            address: Some(17),
            invert_pan: true,
            position_calibration: Some(InstalledPositionCalibration {
                pan_zero_degrees: 37.,
                ..Default::default()
            }),
            ..Default::default()
        });
        let (engine, programmers, session) = engine(fixture.clone());
        if width == 4 {
            for (attribute, raw) in [("pan", 0xfedc_ba98), ("tilt", 0x1234_5679)] {
                programmers.set(
                    session,
                    root,
                    AttributeKey(attribute.into()),
                    AttributeValue::RawDmxExact(raw),
                );
            }
        } else {
            set(&programmers, session, root, "pan", 0.234);
            set(&programmers, session, root, "tilt", 0.678);
        }
        let before = engine.render(Default::default()).unwrap();
        let held_root = words(&before, root.0);
        let held_copy = words(&before, copy);
        if width == 2 {
            assert_ne!(
                held_root, held_copy,
                "copy inversion is captured independently"
            );
        }
        if width == 4 {
            assert_eq!(held_root, [0xfedc_ba98, 0x1234_5679]);
        }
        let axes = before
            .physical
            .instances
            .iter()
            .map(|i| i.axes().to_vec())
            .collect::<Vec<_>>();
        capture_hold(&engine, &mut fixture, false);
        set(&programmers, session, root, "pan", 1.);
        set(&programmers, session, root, "tilt", 0.);
        fixture.location.x += 2500;
        fixture.rotation.z = 45.;
        fixture.multipatch[0].location.y += 1700;
        replace_fixture(&engine, &fixture);
        let frozen = engine.render(Default::default()).unwrap();
        assert_eq!(words(&frozen, root.0), held_root);
        assert_eq!(words(&frozen, copy), held_copy);
        assert_eq!(
            frozen
                .physical
                .instances
                .iter()
                .map(|i| i.axes().to_vec())
                .collect::<Vec<_>>(),
            axes
        );
        assert_dmx(&frozen, 1, width, &held_root);
        assert_dmx(&frozen, 17, width, &held_copy);
        assert_ne!(
            before.mounts.mount(root.0).unwrap().world_from_fixture,
            frozen.mounts.mount(root.0).unwrap().world_from_fixture
        );

        let new_copy = Uuid::new_v4();
        let mut added = fixture.multipatch[0].clone();
        added.id = new_copy;
        added.address = Some(33);
        fixture.multipatch.push(added);
        replace_fixture(&engine, &fixture);
        let expanded = engine.render(Default::default()).unwrap();
        assert_eq!(words(&expanded, root.0), held_root);
        assert_eq!(words(&expanded, copy), held_copy);
        assert_eq!(
            words(&expanded, new_copy),
            [0, 0],
            "new physical bodies never inherit another body's held motor words"
        );
        assert_dmx(&expanded, 33, width, &[0, 0]);

        // Removing just the optional payload must release native motors; family metadata alone
        // cannot manufacture an accepted pose or retain an old cache entry.
        fixture
            .freeze
            .targets
            .get_mut(&root)
            .unwrap()
            .position_native = None;
        replace_fixture(&engine, &fixture);
        let resumed = engine.render(Default::default()).unwrap();
        let max = if width == 2 { 65535 } else { u32::MAX };
        assert_eq!(words(&resumed, root.0), [max, 0]);
        assert_eq!(words(&resumed, copy), [0, 0]);
        assert_dmx(&resumed, 1, width, &[max, 0]);
        fixture.freeze.targets.clear();
        replace_fixture(&engine, &fixture);
        assert_eq!(
            words(&engine.render(Default::default()).unwrap(), root.0),
            [max, 0]
        );
    }
}

#[test]
fn native_freeze_survives_color_and_repatch_but_stale_physics_releases_whole_pair() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    let (engine, programmers, session) = engine(fixture.clone());
    set(&programmers, session, root, "pan", 0.2);
    set(&programmers, session, root, "tilt", 0.7);
    let accepted = engine.render(Default::default()).unwrap();
    let held = words(&accepted, root.0);
    capture_hold(&engine, &mut fixture, false);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, root, "tilt", 0.);
    redefine(&mut fixture, |profile| {
        profile.name = "Same mechanics with an additional emitter".into();
        profile.revision += 1;
        let mode = &mut profile.modes[0];
        let mut color = mode.channels[0].clone();
        color.id = Uuid::new_v4();
        color.attribute = AttributeKey("color.red".into());
        color.fixture_attribute = color.attribute.clone();
        color.resolution = ChannelResolution::U8;
        color.secondary_slots.clear();
        color.default_raw = 0;
        color.highlight_raw = 255;
        color.functions = vec![ChannelFunction::continuous(
            "Red",
            color.attribute.clone(),
            255,
        )];
        mode.channels.push(color);
        mode.splits[0].footprint = 5;
    });
    fixture.address = Some(41);
    replace_fixture(&engine, &fixture);
    set(&programmers, session, root, "color.red", 1.);
    let recolored = engine.render(Default::default()).unwrap();
    assert_eq!(&words(&recolored, root.0)[..2], held.as_slice());
    assert_dmx(&recolored, 41, 2, &held);
    assert_eq!(recolored.universes[&1][44], 255);
    for change in 0..2 {
        let mut stale = fixture.clone();
        if change == 0 {
            stale.position_calibration = Some(InstalledPositionCalibration {
                pan_zero_degrees: 25.,
                ..Default::default()
            });
        } else {
            redefine(&mut stale, |profile| {
                if let ChannelFunctionBehavior::Continuous { physical_max, .. } =
                    &mut profile.modes[0].channels[0].functions[0].behavior
                {
                    *physical_max = 900.;
                }
            });
        }
        replace_fixture(&engine, &stale);
        let resumed = engine.render(Default::default()).unwrap();
        assert_eq!(
            &words(&resumed, root.0)[..2],
            &[65535, 0],
            "one incompatible motor releases the whole owner/instance pair ({change})"
        );
        assert_dmx(&resumed, 41, 2, &[65535, 0]);
    }
}

#[test]
fn native_freeze_keeps_motor_pose_while_tracked_mount_moves() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    let (mut point, point_id) = schema_v2_fixture(&[
        ("point.position.x", false, false),
        ("point.position.y", false, false),
        ("point.position.z", false, false),
    ]);
    point.fixture_number = None;
    point.universe = None;
    point.address = None;
    fixture.position_master = Some(point_id.0);
    let (engine, programmers, session) = engine(fixture.clone());
    let mut snapshot = engine.snapshot().as_ref().clone();
    Arc::make_mut(&mut snapshot.fixtures).push(point);
    engine.replace_snapshot(snapshot).unwrap();
    let track = |x| {
        engine.set_tracked_overrides([
            crate::TrackedOverride::new(
                point_id,
                AttributeKey("point.position.x".into()),
                AttributeValue::Normalized(x),
            ),
            crate::TrackedOverride::new(
                point_id,
                AttributeKey("point.position.y".into()),
                AttributeValue::Normalized(0.5),
            ),
            crate::TrackedOverride::new(
                point_id,
                AttributeKey("point.position.z".into()),
                AttributeValue::Normalized(0.5),
            ),
        ])
    };
    track(0.5);
    set(&programmers, session, root, "pan", 0.3);
    set(&programmers, session, root, "tilt", 0.6);
    capture_hold(&engine, &mut fixture, false);
    let before = engine.render(Default::default()).unwrap();
    let held = words(&before, root.0);
    let local = before
        .physical
        .instances
        .iter()
        .find(|i| i.instance_id == root.0)
        .unwrap()
        .lenses()
        .to_vec();
    set(&programmers, session, root, "pan", 1.);
    track(0.6);
    let moved = engine.render(Default::default()).unwrap();
    assert_eq!(words(&moved, root.0), held);
    assert_dmx(&moved, 1, 2, &held);
    assert_eq!(
        moved
            .physical
            .instances
            .iter()
            .find(|i| i.instance_id == root.0)
            .unwrap()
            .lenses(),
        local
    );
    let old_mount = before.mounts.mount(root.0).unwrap();
    let new_mount = moved.mounts.mount(root.0).unwrap();
    assert_eq!(new_mount.position_master, Some(point_id));
    assert_ne!(
        old_mount.world_from_fixture, new_mount.world_from_fixture,
        "tracking still changes world placement while motor output is held"
    );
}

#[test]
fn partial_native_freeze_obeys_safety_while_full_freeze_bypasses_it() {
    for full in [false, true] {
        let mut fixture = mover();
        let root = fixture.fixture_id;
        fixture.definition.hazardous = true;
        fixture.definition.signal_loss_policy = SignalLossPolicy::ImmediateSafe;
        fixture.definition.safe_values = BTreeMap::from([
            (AttributeKey("pan".into()), AttributeValue::Normalized(0.)),
            (AttributeKey("tilt".into()), AttributeValue::Normalized(1.)),
        ]);
        let (engine, programmers, session) = engine(fixture.clone());
        set(&programmers, session, root, "pan", 0.75);
        set(&programmers, session, root, "tilt", 0.25);
        let held = words(&engine.render(Default::default()).unwrap(), root.0);
        capture_hold(&engine, &mut fixture, full);
        set(&programmers, session, root, "pan", 0.1);
        set(&programmers, session, root, "tilt", 0.9);
        for options in [
            RenderOptions {
                control_loss_progress: Some(1.),
                ..Default::default()
            },
            RenderOptions {
                blackout: true,
                ..Default::default()
            },
            RenderOptions {
                control_loss_progress: Some(1.),
                blackout: true,
                grand_master: 0.,
                ..Default::default()
            },
        ] {
            let output = engine.render(options).unwrap();
            let expected = if full { held.clone() } else { vec![0, 65535] };
            assert_eq!(words(&output, root.0), expected, "Full={full}");
            assert_dmx(&output, 1, 2, &expected);
        }
        assert_eq!(
            words(&engine.render(Default::default()).unwrap(), root.0),
            held,
            "safety overlays do not overwrite the accepted hold"
        );
    }
}
