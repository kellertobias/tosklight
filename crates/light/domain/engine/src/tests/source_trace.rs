use super::*;
use std::collections::HashSet;

#[test]
fn observed_source_identity_distinguishes_equal_group_values_and_sample_replacements() {
    let registry = ProgrammerRegistry::with_clock(Arc::new(ManualClock::new(Utc::now())));
    let session = SessionId::new();
    let programmer = registry.start(session).id;
    let fixture = FixtureId::new();
    let engine = Engine::new(registry.clone());
    let color = AttributeKey::color();
    let payload = AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.));
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![calibrated_visual_fixture(fixture)].into(),
            groups: ["a", "b"]
                .into_iter()
                .map(|id| GroupDefinition {
                    id: id.into(),
                    name: id.into(),
                    fixtures: vec![fixture],
                    ..Default::default()
                })
                .collect::<Vec<_>>()
                .into(),
            ..Default::default()
        })
        .unwrap();
    registry.set_group(session, "a".into(), color.clone(), payload.clone());
    registry.set_group(session, "b".into(), color.clone(), payload.clone());
    let capture = engine.prepare_observer_frame(Default::default());
    let before = engine.observe_prepared_frame(&capture, &[]);
    let origin = before
        .values()
        .contribution_origin(fixture, &color)
        .unwrap();
    assert_eq!(
        origin.source(),
        &ContributionSourceId::programmer_group(programmer, "b")
    );
    let releases = ContributionBatch::releasing([(
        origin.source().clone(),
        fixture,
        color.clone(),
        ContributionReleaseCutoff {
            changed_at: origin.stamp().changed_at,
            programmer_order: origin.stamp().programmer_order + 1,
        },
    )]);
    let after = engine.observe_prepared_frame(&capture, std::slice::from_ref(&releases));
    assert_eq!(
        before.values().value(fixture, &color),
        after.values().value(fixture, &color)
    );
    assert_eq!(
        after
            .values()
            .contribution_origin(fixture, &color)
            .unwrap()
            .source(),
        &ContributionSourceId::programmer_group(programmer, "a")
    );
    let same_value_release = engine
        .profile_color_release_frames(
            &before,
            &after,
            &releases,
            Default::default(),
            &HashSet::new(),
        )
        .unwrap();
    assert_eq!(
        &*same_value_release.native_ownership[&fixture],
        &[false, true, true, true]
    );
    // Releasing the shadowed Group A must acquire neither its channels nor Group B's channels.
    let shadowed = ContributionBatch::excluding([(
        ContributionSourceId::programmer_group(programmer, "a"),
        fixture,
        color.clone(),
    )]);
    let unaffected = engine.observe_prepared_frame(&capture, std::slice::from_ref(&shadowed));
    assert!(
        engine
            .profile_color_release_frames(
                &before,
                &unaffected,
                &shadowed,
                Default::default(),
                &HashSet::new()
            )
            .unwrap()
            .native_ownership
            .is_empty()
    );
    let mut value = registry.get(session).unwrap().group_values["b"][&color].clone();
    value.value = payload.clone();
    let sampled = ContributionBatch::new([ContributionSample::replacing(
        TimedValue {
            fixture_id: fixture,
            attribute: color.clone(),
            value: payload,
            priority: 1000,
            changed_at: value.changed_at,
            programmer_order: value.programmer_order,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        },
        ContributionSourceId::preload(programmer),
    )]);
    let sample_frame = engine.observe_source_frame(&[sampled]);
    assert_eq!(
        sample_frame
            .values()
            .contribution_origin(fixture, &color)
            .unwrap()
            .source(),
        &ContributionSourceId::preload(programmer)
    );
    let normal = engine.render(Default::default()).unwrap();
    assert!(
        normal
            .resolved_values
            .contribution_origin(fixture, &color)
            .is_none()
    );
}

#[test]
fn color_release_union_resets_consumed_uv_and_wheel_without_claiming_independent_controls() {
    let registry = ProgrammerRegistry::with_clock(Arc::new(ManualClock::new(Utc::now())));
    let session = SessionId::new();
    let programmer = registry.start(session).id;
    let fixture = FixtureId::new();
    let mut patched = calibrated_visual_fixture(fixture);
    let mut profile = patched
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = &mut profile.modes[0];
    let head = mode.heads[0].id;
    let uv = uuid::Uuid::new_v4();
    mode.channels
        .push(calibrated_channel(uv, head, "color.uv", false));
    if let ColorSystem::Additive { emitters } = &mut mode.color_systems[0].system {
        let mut emitter = emitter(
            uv,
            "UV",
            Xyz {
                x: 0.,
                y: 0.,
                z: 0.,
            },
            1.,
        );
        emitter.visible = false;
        emitters.push(emitter);
    }
    for name in ["color.wheel", "color.wheel.2"] {
        let wheel = uuid::Uuid::new_v4();
        mode.channels
            .push(calibrated_channel(wheel, head, name, false));
        mode.color_systems.push(light_fixture::HeadColorSystem {
            calibration: Default::default(),
            head_id: head,
            correction_matrix: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            system: ColorSystem::DiscreteWheel {
                channel_id: wheel,
                slots: vec![light_fixture::ColorWheelSlot {
                    semantic_id: "open".into(),
                    label: "Open".into(),
                    dmx_from: 0,
                    dmx_to: 10,
                    measured_xyz: Some(light_core::color_intent::D65_WHITE),
                    steady: Some(true),
                }],
            },
        });
    }
    mode.splits[0].footprint = 7;
    let second_wheel = light_fixture::FixtureMode::control_action_attribute(mode.channels[6].id);
    patched.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            ..Default::default()
        })
        .unwrap();
    engine.set_color_model(light_core::ColorProgrammingModel::Intent);
    registry.set_many(
        session,
        [
            (
                fixture,
                AttributeKey::color(),
                AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.)),
            ),
            (fixture, second_wheel, AttributeValue::RawDmxExact(19)),
            (
                fixture,
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.6),
            ),
        ],
    );
    let capture = engine.prepare_observer_frame(Default::default());
    let before = engine.observe_prepared_frame(&capture, &[]);
    let releases = ContributionBatch::excluding([(
        ContributionSourceId::programmer(programmer),
        fixture,
        AttributeKey::color(),
    )]);
    let after = engine.observe_prepared_frame(&capture, std::slice::from_ref(&releases));
    let result = engine
        .profile_color_release_frames(
            &before,
            &after,
            &releases,
            Default::default(),
            &HashSet::new(),
        )
        .unwrap();
    assert_eq!(
        &*result.native_ownership[&fixture],
        &[false, true, true, true, true, true, false]
    );
    assert_eq!(result.physical.instances[0].native_raw[6], 19);
    let expected = engine
        .profile_observed_source_frame(&after, Default::default(), &HashSet::new())
        .unwrap();
    assert_eq!(
        result.physical.instances[0].native_raw,
        expected.physical.instances[0].native_raw
    );
    assert_eq!(
        registry.get(session).unwrap().values.len(),
        3,
        "observation cannot mutate programming"
    );
}

#[test]
fn release_frame_pair_rejects_different_times_or_generations() {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let before = engine.observe_source_frame(&[]);
    clock.advance_millis(1);
    let different_time = engine.observe_source_frame(&[]);
    assert!(
        engine
            .profile_color_release_frames(
                &before,
                &different_time,
                &ContributionBatch::default(),
                Default::default(),
                &HashSet::new()
            )
            .is_err()
    );
    let before = engine.observe_source_frame(&[]);
    engine
        .replace_snapshot(EngineSnapshot {
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let different_generation = engine.observe_source_frame(&[]);
    assert!(
        engine
            .profile_color_release_frames(
                &before,
                &different_generation,
                &ContributionBatch::default(),
                Default::default(),
                &HashSet::new()
            )
            .is_err()
    );

    // A clock tick and show replacement are not the only ways inputs can diverge. Two captures
    // with the same timestamp and generation can contain different Programmer source states.
    let registry = ProgrammerRegistry::with_clock(clock);
    let session = SessionId::new();
    registry.start(session);
    let fixture = FixtureId::new();
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![calibrated_visual_fixture(fixture)].into(),
            ..Default::default()
        })
        .unwrap();
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.25),
    );
    let first_capture = engine.observe_source_frame(&[]);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.75),
    );
    let second_capture = engine.observe_source_frame(&[]);
    assert_eq!(first_capture.sampled_at(), second_capture.sampled_at());
    assert!(std::sync::Arc::ptr_eq(
        &first_capture.snapshot(),
        &second_capture.snapshot()
    ));
    assert_ne!(
        first_capture
            .values()
            .value(fixture, &AttributeKey::intensity()),
        second_capture
            .values()
            .value(fixture, &AttributeKey::intensity())
    );
    assert!(
        engine
            .profile_color_release_frames(
                &first_capture,
                &second_capture,
                &ContributionBatch::default(),
                Default::default(),
                &HashSet::new()
            )
            .is_err()
    );
}

#[test]
fn source_frame_projection_keeps_the_captured_cue_master_after_live_values_change() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    let fixture = FixtureId::new();
    let patched = calibrated_visual_fixture(fixture);
    let color = AttributeKey::color();
    let cue = test_cue_list(
        "Source frame",
        vec![
            CueChange::set(
                fixture,
                AttributeKey::intensity(),
                AttributeValue::Normalized(1.),
            ),
            CueChange::set(
                fixture,
                color.clone(),
                AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.)),
            ),
        ],
    );
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            cue_lists: vec![cue.clone()].into(),
            playbacks: vec![test_playback(1, cue.id)].into(),
            ..Default::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    execute_pool(&engine, 1, PoolPlaybackAction::SetVirtualMaster(0.25));
    let live = engine.render(Default::default()).unwrap();
    let frame = engine.observe_source_frame(&[]);
    assert!(
        frame
            .values()
            .contribution_origin(fixture, &color)
            .is_some()
    );
    registry.set_many(
        session,
        [(
            fixture,
            color.clone(),
            AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(0., 1., 0.)),
        )],
    );
    let projected = engine
        .profile_observed_source_frame(
            &frame,
            Default::default(),
            &HashSet::from([(fixture, color)]),
        )
        .unwrap();
    assert_eq!(
        projected.physical.instances[0].native_raw,
        live.physical.instances[0].native_raw
    );
    assert!(
        projected.native_ownership[&fixture]
            .iter()
            .any(|owned| *owned)
    );
}

#[test]
fn whole_color_freeze_clears_the_released_source_and_excludes_preview_ownership() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    let fixture = FixtureId::new();
    let color = AttributeKey::color();
    let payload = AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.));
    let mut patched = calibrated_visual_fixture(fixture);
    patched.freeze = FixtureFreezeState {
        targets: HashMap::from([(
            fixture,
            FrozenFixtureTarget {
                position_native: None,
                full: false,
                families: vec![FreezeFamily::Color],
                values: HashMap::from([(color.clone(), payload.clone())]),
            },
        )]),
    };
    let engine = Engine::new(registry.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            ..Default::default()
        })
        .unwrap();
    registry.set_many(session, [(fixture, color.clone(), payload)]);
    let frame = engine.observe_source_frame(&[]);
    assert!(
        frame
            .values()
            .contribution_origin(fixture, &color)
            .is_none()
    );
    let projection = engine
        .profile_observed_source_frame(
            &frame,
            Default::default(),
            &HashSet::from([(fixture, color)]),
        )
        .unwrap();
    assert!(!projection.native_ownership.contains_key(&fixture));
}
